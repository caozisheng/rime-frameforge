import type { RawFrameDescriptor } from '../contracts.js';
import { drcPipelineWgsl } from '../generated/drc_pipeline.generated.js';

const LUT_SAMPLES = 257;
const TILES_X = 8;
const TILES_Y = 6;
const HISTOGRAM_BINS = 64;
const MODULATION_SAMPLES = 64;
const LEVEL_COUNT = 3;
export const DRC_PIPELINE_WGSL = drcPipelineWgsl;

export interface DrcModulationCurves {
  readonly edge: readonly (readonly [number, number])[];
  readonly luma: readonly (readonly [number, number])[];
}

export const REFERENCE_EDGE_CURVE: readonly (readonly [number, number])[] = [[0, 1], [0.1, 0.9], [0.2, 0.7], [0.3, 0.5], [0.4, 0.3], [0.5, 0.2], [0.8, 0], [1, 0]];
export const REFERENCE_LUMA_CURVE: readonly (readonly [number, number])[] = [[0, 0], [0.2, 0.3], [0.3, 0.6], [0.5, 0.7], [0.7, 0.8], [1, 1]];
export const DEFAULT_MODULATION_CURVES: Readonly<DrcModulationCurves> = Object.freeze({ edge: REFERENCE_EDGE_CURVE, luma: REFERENCE_LUMA_CURVE });

export function validateModulationCurves(curves: DrcModulationCurves): void {
  for (const curve of [curves.edge, curves.luma]) {
    if (curve.length < 2 || curve.some(([x, y]) => !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || x > 1 || y < 0 || y > 1)
      || curve.some((point, index) => index > 0 && curve[index - 1]![0] >= point[0])) {
      throw new Error('DRC_MODULATION_CURVE_INVALID: knots must be finite, x in [0,1] strictly ascending, y in [0,1]');
    }
  }
}

export function bakeModulationLuts(curves: DrcModulationCurves = DEFAULT_MODULATION_CURVES): Float32Array<ArrayBuffer> {
  validateModulationCurves(curves);
  const bake = (curve: readonly (readonly [number, number])[]): number[] => Array.from({ length: MODULATION_SAMPLES }, (_, index) => {
    const x = index / (MODULATION_SAMPLES - 1);
    let upper = curve.findIndex((point) => point[0] >= x);
    if (upper <= 0) return curve[upper === -1 ? curve.length - 1 : 0]![1];
    const [x0, y0] = curve[upper - 1]!;
    const [x1, y1] = curve[upper]!;
    const t = x1 === x0 ? 1 : (x - x0) / (x1 - x0);
    return y0 + t * (y1 - y0);
  });
  return new Float32Array([...bake(curves.edge), ...bake(curves.luma)]);
}

export type DrcMethod = '00' | '01';

export interface DrcIqParameters {
  readonly drc_gain_offset_ev: number;
  readonly knee: number;
  readonly amplifier: number;
}

export const DEFAULT_DRC_IQ_PARAMETERS: DrcIqParameters = Object.freeze({
  drc_gain_offset_ev: 0,
  knee: 1,
  amplifier: 3,
});

export function validateDrcIqParameters(parameters: DrcIqParameters): void {
  if (!Number.isFinite(parameters.drc_gain_offset_ev) || parameters.drc_gain_offset_ev < -4 || parameters.drc_gain_offset_ev > 4
    || !Number.isFinite(parameters.knee) || parameters.knee <= 0
    || !Number.isFinite(parameters.amplifier) || parameters.amplifier < 0) {
    throw new Error('DRC_IQ_INVALID: offset must be finite in [-4, 4], knee must be finite and positive, and amplifier must be finite and non-negative');
  }
}
export function packDrcUniforms(descriptor: Pick<RawFrameDescriptor, 'baselineExposure'>, parameters: DrcIqParameters): ArrayBuffer {
  validateDrcIqParameters(parameters);
  const metadataGain = 2 ** Math.max(0, descriptor.baselineExposure ?? 0);
  const finalGain = Math.max(1, metadataGain * 2 ** parameters.drc_gain_offset_ev);
  const packed = new ArrayBuffer(32);
  const view = new DataView(packed);
  view.setFloat32(0, finalGain, true);
  view.setFloat32(4, parameters.knee, true);
  view.setFloat32(8, parameters.amplifier, true);
  view.setFloat32(12, 1 / 65_536, true);
  view.setFloat32(16, 1 / 256, true);
  view.setFloat32(20, finalGain * 4, true);
  view.setUint32(24, LEVEL_COUNT, true);
  view.setUint32(28, 1 | (TILES_X << 8) | (TILES_Y << 16), true);
  return packed;
}
type DrcPipelineKey =
  | 'drc_prefilter_main'
  | 'pyramid_downsample_main'
  | 'pyramid_reconstruct_main'
  | 'guided_coefficients_main'
  | 'guided_apply_vertical_main'
  | 'drc_combine_global_main'
  | 'drc_combine_local_main';

interface DrcLevelResources {
  readonly level: GPUTexture;
  readonly candidate: GPUTexture | null;
  readonly base: GPUTexture;
  readonly coefficients: GPUTexture;
}

export class WebDrcExecutor {
  readonly #device: GPUDevice;
  readonly #uniform: GPUBuffer;
  readonly #globalLut: GPUBuffer;
  readonly #localLut: GPUBuffer;
  readonly #modulationLuts: GPUBuffer;
  readonly #pipelines: Readonly<Record<DrcPipelineKey, GPUComputePipeline>>;
  readonly #levels: readonly DrcLevelResources[];
  #descriptor: RawFrameDescriptor;
  #raw: Uint16Array;
  #method: DrcMethod = '00';
  #modulationCurves: DrcModulationCurves = DEFAULT_MODULATION_CURVES;

  public constructor(device: GPUDevice, raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor) {
    this.#device = device;
    this.#descriptor = descriptor;
    this.#raw = rawView(raw, rawByteOffset, descriptor);
    this.#uniform = device.createBuffer({ label: 'drc-web-scalars', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#globalLut = device.createBuffer({ label: 'drc-web-global-lut', size: LUT_SAMPLES * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.#localLut = device.createBuffer({ label: 'drc-web-local-lut', size: LUT_SAMPLES * TILES_X * TILES_Y * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.#modulationLuts = device.createBuffer({ label: 'drc-web-modulation-luts', size: MODULATION_SAMPLES * 2 * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    const module = device.createShaderModule({ label: 'drc-web-pipeline', code: drcPipelineWgsl });
    const pipeline = (entryPoint: DrcPipelineKey): GPUComputePipeline => device.createComputePipeline({ label: entryPoint, layout: 'auto', compute: { module, entryPoint } });
    this.#pipelines = {
      drc_prefilter_main: pipeline('drc_prefilter_main'), pyramid_downsample_main: pipeline('pyramid_downsample_main'), pyramid_reconstruct_main: pipeline('pyramid_reconstruct_main'),
      guided_coefficients_main: pipeline('guided_coefficients_main'),
      guided_apply_vertical_main: pipeline('guided_apply_vertical_main'),
      drc_combine_global_main: pipeline('drc_combine_global_main'), drc_combine_local_main: pipeline('drc_combine_local_main'),
    };
    this.#levels = levelExtents(descriptor.width, descriptor.height).map(([width, height], index, extents) => ({
      level: createTexture(device, `drc-web-level-${index}`, 'r32float', width, height),
      candidate: index === extents.length - 1 ? null : createTexture(device, `drc-web-candidate-${index}`, 'r32float', width, height),
      base: createTexture(device, `drc-web-base-${index}`, 'r32float', width, height),
      coefficients: createTexture(device, `drc-web-coeff-${index}`, 'rgba16float', width, height),
    }));
  }

  #iqParameters: DrcIqParameters = DEFAULT_DRC_IQ_PARAMETERS;

  public replaceFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    this.#descriptor = descriptor;
    this.#raw = rawView(raw, rawByteOffset, descriptor);
  }
  public setIqParameters(parameters: DrcIqParameters, curves: DrcModulationCurves = this.#modulationCurves): void {
    validateDrcIqParameters(parameters);
    validateModulationCurves(curves);
    this.#iqParameters = { ...parameters };
    this.#modulationCurves = { edge: curves.edge.map(([x, y]) => [x, y]), luma: curves.luma.map(([x, y]) => [x, y]) };
  }

  public setMethod(method: DrcMethod): void {
    this.#method = method;
  }

  public prepare(): void {
    const gain = resolveDrcGain(this.#descriptor, this.#iqParameters);
    const global = generateGlobalToneLut(gain, this.#iqParameters.knee);
    const local = this.#method === '01' ? generateLocalToneLuts(this.#raw, this.#descriptor, global) : null;
    this.#device.queue.writeBuffer(this.#uniform, 0, packDrcUniforms(this.#descriptor, this.#iqParameters));
    this.#device.queue.writeBuffer(this.#globalLut, 0, global.buffer as ArrayBuffer, global.byteOffset, global.byteLength);
    if (local !== null) this.#device.queue.writeBuffer(this.#localLut, 0, local.buffer as ArrayBuffer, local.byteOffset, local.byteLength);
    this.#device.queue.writeBuffer(this.#modulationLuts, 0, bakeModulationLuts(this.#modulationCurves));
  }


  public encode(encoder: GPUCommandEncoder, input: GPUTexture, output: GPUTexture): void {
    const first = this.#levels[0]!;
    this.encodePass(encoder, 'drc_prefilter_main', [[1, input]], 4, first.level, []);
    for (let index = 1; index < this.#levels.length; index += 1) {
      this.encodePass(encoder, 'pyramid_downsample_main', [[1, this.#levels[index - 1]!.level]], 4, this.#levels[index]!.level, []);
    }
    let base = this.encodeGuided(encoder, this.#levels.length - 1, this.#levels.at(-1)!.level);
    for (let index = this.#levels.length - 2; index >= 0; index -= 1) {
      const current = this.#levels[index]!;
      this.encodePass(encoder, 'pyramid_reconstruct_main', [[1, current.level], [2, this.#levels[index + 1]!.level], [3, base]], 4, current.candidate!, []);
      base = this.encodeGuided(encoder, index, current.candidate!);
    }
    const buffers: readonly [number, GPUBuffer][] = this.#method === '01' ? [[6, this.#globalLut], [7, this.#localLut], [8, this.#modulationLuts]] : [[6, this.#globalLut], [8, this.#modulationLuts]];
    const combine = this.#method === '01' ? 'drc_combine_local_main' : 'drc_combine_global_main';
    this.encodePass(encoder, combine, [[1, input], [2, first.level], [3, base]], 4, output, buffers);
  }
  public dispose(): void {
    this.#uniform.destroy();
    this.#globalLut.destroy();
    this.#localLut.destroy();
    this.#modulationLuts.destroy();
    for (const level of this.#levels) {
      level.level.destroy();
      level.candidate?.destroy();
      level.base.destroy();
      level.coefficients.destroy();
    }
  }

  private encodeGuided(encoder: GPUCommandEncoder, index: number, input: GPUTexture): GPUTexture {
    const resources = this.#levels[index]!;
    this.encodePass(encoder, 'guided_coefficients_main', [[1, input]], 5, resources.coefficients, []);
    this.encodePass(encoder, 'guided_apply_vertical_main', [[1, resources.coefficients], [2, input]], 4, resources.base, [[8, this.#modulationLuts]]);
    return resources.base;
  }

  private encodePass(
    encoder: GPUCommandEncoder,
    entryPoint: DrcPipelineKey,
    inputs: readonly [number, GPUTexture][],
    outputBinding: number,
    output: GPUTexture,
    buffers: readonly [number, GPUBuffer][],
  ): void {
    const pipeline = this.#pipelines[entryPoint];
    const entries: GPUBindGroupEntry[] = [
      { binding: 0, resource: { buffer: this.#uniform } },
      ...inputs.map(([binding, texture]) => ({ binding, resource: texture.createView() })),
      { binding: outputBinding, resource: output.createView() },
      ...buffers.map(([binding, buffer]) => ({ binding, resource: { buffer } })),
    ];
    const bindGroup = this.#device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries });
    const pass = encoder.beginComputePass({ label: entryPoint });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(Math.ceil(output.width / 8), Math.ceil(output.height / 8));
    pass.end();
  }
}

function resolveDrcGain(descriptor: Pick<RawFrameDescriptor, 'baselineExposure'>, parameters: DrcIqParameters): number {
  return Math.max(1, 2 ** Math.max(0, descriptor.baselineExposure ?? 0) * 2 ** parameters.drc_gain_offset_ev);
}

export function generateGlobalToneLut(gain: number, knee = 1): Float32Array {
  if (!Number.isFinite(gain) || gain <= 0 || !Number.isFinite(knee) || knee <= 0) throw new Error('DRC_TONE_INVALID: gain and knee must be finite and positive');
  const curve = Array.from({ length: LUT_SAMPLES }, (_, index) => {
    const t = index / (LUT_SAMPLES - 1);
    return { x: 2 * (1 - t) * t * knee / gain + t * t, y: 2 * (1 - t) * t * knee + t * t };
  });
  const output = new Float32Array(LUT_SAMPLES);
  let segment = 0;
  for (let index = 0; index < output.length; index += 1) {
    const x = index / (output.length - 1);
    while (segment + 1 < curve.length && curve[segment + 1]!.x < x) segment += 1;
    const left = curve[segment]!;
    const right = curve[Math.min(segment + 1, curve.length - 1)]!;
    const fraction = right.x <= left.x ? 1 : (x - left.x) / (right.x - left.x);
    output[index] = left.y * (1 - fraction) + right.y * fraction;
  }
  output[0] = 0;
  output[output.length - 1] = 1;
  return output;
}
function generateLocalToneLuts(raw: Uint16Array, descriptor: RawFrameDescriptor, global: Float32Array): Float32Array {
  const histograms = new Uint32Array(TILES_X * TILES_Y * HISTOGRAM_BINS);
  const range = descriptor.whiteLevel - descriptor.blackLevel;
  const cfa = cfaPattern(descriptor.cfa);
  const cfaGains = cfa.map((channel) => descriptor.whiteBalanceGains[channel] ?? 1);
  const cfaGainAvg = cfaGains.reduce((sum: number, gain: number) => sum + gain, 0) / cfaGains.length;
  const normalizedGains = cfaGains.map((gain) => gain / cfaGainAvg);
  for (let y = 0; y < descriptor.height; y += 1) {
    for (let x = 0; x < descriptor.width; x += 1) {
      const normalized = Math.max(0, Math.min(1, bayerLuma3x3(raw, descriptor, x, y, range, cfa, normalizedGains)));
      const tile = Math.min(TILES_Y - 1, Math.floor(y * TILES_Y / descriptor.height)) * TILES_X + Math.min(TILES_X - 1, Math.floor(x * TILES_X / descriptor.width));
      const bin = Math.min(HISTOGRAM_BINS - 1, Math.floor(normalized * (HISTOGRAM_BINS - 1)));
      histograms[tile * HISTOGRAM_BINS + bin]! += 1;
    }
  }

  const output = new Float32Array(TILES_X * TILES_Y * LUT_SAMPLES);
  for (let tile = 0; tile < TILES_X * TILES_Y; tile += 1) {
    const histogram = histograms.subarray(tile * HISTOGRAM_BINS, (tile + 1) * HISTOGRAM_BINS);
    const total = histogram.reduce((sum, value) => sum + value, 0);
    const occupied = histogram.reduce((sum, value) => sum + Number(value > 0), 0);
    const target = output.subarray(tile * LUT_SAMPLES, (tile + 1) * LUT_SAMPLES);
    if (total < 16 || occupied <= 1) { target.set(global); continue; }
    const mean = histogram.reduce((sum, value, index) => sum + index / (HISTOGRAM_BINS - 1) * value, 0) / total;
    const variance = histogram.reduce((sum, value, index) => {
      const difference = index / (HISTOGRAM_BINS - 1) - mean;
      return sum + difference * difference * value;
    }, 0) / total;
    const sampleConfidence = Math.max(0, Math.min(1, total / 16 / 4));
    const spreadConfidence = Math.max(0, Math.min(1, Math.sqrt(variance) * 8));
    const localWeight = 0.75 * sampleConfidence * spreadConfidence;
    const low = Math.floor(total / 100);
    const high = total - low;
    for (let index = 0; index < LUT_SAMPLES; index += 1) {
      const bin = Math.floor(index * (HISTOGRAM_BINS - 1) / (LUT_SAMPLES - 1));
      let cumulative = 0;
      for (let current = 0; current <= bin; current += 1) cumulative += histogram[current] ?? 0;
      const cdf = (Math.max(low, Math.min(high, cumulative)) - low) / Math.max(1, high - low);
      const mapped = sampleLut(global, cdf);
      target[index] = global[index]! * (1 - localWeight) + mapped * localWeight;
    }
    target[0] = 0;
    target[target.length - 1] = 1;
    for (let index = 1; index < target.length; index += 1) target[index] = Math.max(target[index]!, target[index - 1]!);
  }
  return gaussianSmoothTiles(output);
}

function bayerLuma3x3(raw: Uint16Array, descriptor: RawFrameDescriptor, x: number, y: number, range: number, cfa: readonly number[], normalizedGains?: readonly number[]): number {
  const weights = [1, 2, 1] as const;
  let sum = 0;
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      const sampleX = x + dx;
      const sampleY = y + dy;
      if (sampleX < 0 || sampleY < 0 || sampleX >= descriptor.width || sampleY >= descriptor.height) continue;
      const sample = raw[sampleY * descriptor.rowStrideSamples + sampleX]!;
      const cfaIndex = (sampleY & 1) * 2 + (sampleX & 1);
      const channel = cfa[cfaIndex] ?? 1;
      const gain = normalizedGains?.[cfaIndex] ?? descriptor.whiteBalanceGains[channel] ?? 1;
      sum += ((sample - descriptor.blackLevel) / range) * gain * weights[dx + 1]! * weights[dy + 1]!;
    }
  }
  return sum / 16;
}

function gaussianSmoothTiles(values: Float32Array): Float32Array {
  const output = new Float32Array(values.length);
  for (let tileY = 0; tileY < TILES_Y; tileY += 1) {
    for (let tileX = 0; tileX < TILES_X; tileX += 1) {
      for (let sample = 0; sample < LUT_SAMPLES; sample += 1) {
        let sum = 0;
        let weightSum = 0;
        for (let dy = -1; dy <= 1; dy += 1) {
          for (let dx = -1; dx <= 1; dx += 1) {
            const x = Math.max(0, Math.min(TILES_X - 1, tileX + dx));
            const y = Math.max(0, Math.min(TILES_Y - 1, tileY + dy));
            const weight = (dx === 0 ? 2 : 1) * (dy === 0 ? 2 : 1);
            sum += values[(y * TILES_X + x) * LUT_SAMPLES + sample]! * weight;
            weightSum += weight;
          }
        }
        output[(tileY * TILES_X + tileX) * LUT_SAMPLES + sample] = sum / weightSum;
      }
    }
  }
  return output;
}

function sampleLut(values: Float32Array, input: number): number {
  const position = Math.max(0, Math.min(1, input)) * (values.length - 1);
  const lower = Math.floor(position);
  const upper = Math.min(lower + 1, values.length - 1);
  return values[lower]! * (1 - (position - lower)) + values[upper]! * (position - lower);
}

function levelExtents(width: number, height: number): readonly [number, number][] {
  const levels: [number, number][] = [];
  for (let index = 0; index < LEVEL_COUNT; index += 1) {
    levels.push([width, height]);
    width = Math.max(1, Math.floor(width / 2));
    height = Math.max(1, Math.floor(height / 2));
  }
  return levels;
}
function cfaPattern(cfa: RawFrameDescriptor['cfa']): readonly number[] {
  return { rggb: [0, 1, 1, 2], grbg: [1, 0, 2, 1], gbrg: [1, 2, 0, 1], bggr: [2, 1, 1, 0] }[cfa];
}


function createTexture(device: GPUDevice, label: string, format: GPUTextureFormat, width: number, height: number): GPUTexture {
  return device.createTexture({ label, size: [width, height, 1], format, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.STORAGE_BINDING });
}

function rawView(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): Uint16Array {
  const expected = descriptor.rowStrideSamples * descriptor.height;
  return new Uint16Array(raw, rawByteOffset, expected);
}
