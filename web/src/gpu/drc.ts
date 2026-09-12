import { drcPipelineWgsl } from '../generated/drc_pipeline.generated.js';

const LUT_SAMPLES = 257;
const TILES_X = 8;
const TILES_Y = 6;
const MODULATION_SAMPLES = 64;
const LEVEL_COUNT = 3;
export const DRC_PIPELINE_WGSL = drcPipelineWgsl;

export interface DrcModulationCurves {
  readonly edge: readonly (readonly [number, number])[];
  readonly luma: readonly (readonly [number, number])[];
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
export function validateModulationCurves(curves: DrcModulationCurves): void {
  for (const curve of [curves.edge, curves.luma]) {
    if (curve.length < 2 || curve.some(([x, y], index) => !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || x > 1 || y < 0 || y > 1 || (index > 0 && x <= curve[index - 1]![0]))) {
      throw new Error('DRC_MODULATION_INVALID: curves require increasing normalized knots');
    }
  }
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
  #method: DrcMethod = '00';

  public constructor(device: GPUDevice, width: number, height: number) {
    this.#device = device;
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
    this.#levels = levelExtents(width, height).map(([width, height], index, extents) => ({
      level: createTexture(device, `drc-web-level-${index}`, 'r32float', width, height),
      candidate: index === extents.length - 1 ? null : createTexture(device, `drc-web-candidate-${index}`, 'r32float', width, height),
      base: createTexture(device, `drc-web-base-${index}`, 'r32float', width, height),
      coefficients: createTexture(device, `drc-web-coeff-${index}`, 'rgba16float', width, height),
    }));
  }


  public setMethod(method: DrcMethod): void {
    this.#method = method;
  }

  public preparePackets(uniform: Uint8Array<ArrayBuffer>, globalLut: Uint8Array<ArrayBuffer>, localLut: Uint8Array<ArrayBuffer>, modulationLuts: Uint8Array<ArrayBuffer>): void {
    this.#device.queue.writeBuffer(this.#uniform, 0, uniform);
    this.#device.queue.writeBuffer(this.#globalLut, 0, globalLut);
    if (localLut.byteLength > 0) this.#device.queue.writeBuffer(this.#localLut, 0, localLut);
    this.#device.queue.writeBuffer(this.#modulationLuts, 0, modulationLuts);
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

function levelExtents(width: number, height: number): readonly [number, number][] {
  const levels: [number, number][] = [];
  for (let index = 0; index < LEVEL_COUNT; index += 1) {
    levels.push([width, height]);
    width = Math.max(1, Math.floor(width / 2));
    height = Math.max(1, Math.floor(height / 2));
  }
  return levels;
}


function createTexture(device: GPUDevice, label: string, format: GPUTextureFormat, width: number, height: number): GPUTexture {
  return device.createTexture({ label, size: [width, height, 1], format, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.STORAGE_BINDING });
}
