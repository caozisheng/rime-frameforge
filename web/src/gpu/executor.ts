import { normalManifest } from '../generated/normal_manifest.generated.js';
import type { FramePhase, PreviewDescriptor, RawFrameDescriptor, TransferAuditSnapshot } from '../contracts.js';
import type { ExecutionIdentity } from '../runtime-controller.js';
import { resizePreviewCanvas } from '../preview-state.js';
import type { GpuContext } from './device.js';
import { compileFusedNormalShader, compileSegmentedNormalShaders } from './fused-normal-shader.js';
import { FUSED_UNIFORM_BYTES, packFusedUniforms } from './fused-uniforms.js';
import { GpuPreviewPresenter, type PreviewView } from './presenter.js';
import type { QuantizationConfig } from './quantization.js';
import { TransferAudit } from './transfer-audit.js';

const DEM_METHODS = ['00', '01', '02', '03', '04'] as const;
type DemMethod = (typeof DEM_METHODS)[number];
const DEM_ENTRY_POINTS: Record<Exclude<DemMethod, '00'>, string> = {
  '01': 'demosaic_mhc_main',
  '02': 'demosaic_ppg_main',
  '03': 'demosaic_vng_main',
  '04': 'demosaic_ahd_main',
};
const HALF_FLOAT_PREVIEW_NODES = new Set(['dem', 'color_correction', 'gamma', 'rgb2yuv']);
export class NormalGpuExecutor {
  readonly #gpu: GpuContext;
  readonly #presenter: GpuPreviewPresenter;
  readonly #rawTexture: GPUTexture;
  readonly #blcTexture: GPUTexture;
  readonly #wbcTexture: GPUTexture;
  readonly #demTexture: GPUTexture;
  readonly #demIntermediateTexture: GPUTexture;
  readonly #colorTexture: GPUTexture;
  readonly #gammaTexture: GPUTexture;
  readonly #outputTexture: GPUTexture;
  readonly #uniforms: GPUBuffer;
  readonly #previewTextures: Readonly<Record<string, GPUTexture>>;
  #demUniforms: GPUBuffer | null = null;
  #audit = new TransferAudit();
  #descriptor: RawFrameDescriptor;
  #quantizationConfig: QuantizationConfig;
  #demMethod: DemMethod = '00';
  #fullPipeline: GPUComputePipeline | null = null;
  #prePipeline: GPUComputePipeline | null = null;
  #demPipeline: GPUComputePipeline | null = null;
  #demQuantizePipeline: GPUComputePipeline | null = null;
  #postPipeline: GPUComputePipeline | null = null;
  #fullBindGroup: GPUBindGroup | null = null;
  #preBindGroup: GPUBindGroup | null = null;
  #demBindGroup: GPUBindGroup | null = null;
  #demQuantizeBindGroup: GPUBindGroup | null = null;
  #postBindGroup: GPUBindGroup | null = null;
  #committedPreviews: readonly PreviewDescriptor[] = [];
  #sampleBuffer: GPUBuffer | null = null;
  #demosaicParameterValues = { vng_threshold: 1.5, ahd_l_threshold: 2.0, ahd_c_threshold_sq: 4.0 };

  public constructor(gpu: GpuContext, raw: ArrayBuffer, rawByteOffset: number, _generation: number, descriptor: RawFrameDescriptor, quantizationConfig: QuantizationConfig) {
    this.#gpu = gpu;
    this.#descriptor = descriptor;
    this.#quantizationConfig = quantizationConfig;
    this.#presenter = new GpuPreviewPresenter(gpu.context, gpu.device, gpu.canvasFormat);
    const previewUsage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC;
    this.#rawTexture = this.createTexture('normal-raw-source', 'r16uint', GPUTextureUsage.COPY_DST | previewUsage);
    this.#blcTexture = this.createTexture('normal-blc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#wbcTexture = this.createTexture('normal-wbc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demTexture = this.createTexture('normal-dem', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demIntermediateTexture = this.createTexture('normal-dem-intermediate', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING);
    this.#colorTexture = this.createTexture('normal-color', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#gammaTexture = this.createTexture('normal-gamma', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#outputTexture = this.createTexture('normal-yuv', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#previewTextures = {
      raw_source: this.#rawTexture,
      blc: this.#blcTexture,
      wbc: this.#wbcTexture,
      dem: this.#demTexture,
      color_correction: this.#colorTexture,
      gamma: this.#gammaTexture,
      rgb2yuv: this.#outputTexture,
    };
    this.#uniforms = gpu.device.createBuffer({ label: 'normal-fused-params', size: FUSED_UNIFORM_BYTES, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.uploadFrame(raw, rawByteOffset, descriptor);
  }

  public canReplaceFrame(descriptor: RawFrameDescriptor): boolean {
    return descriptor.width === this.#descriptor.width
      && descriptor.height === this.#descriptor.height
      && descriptor.rowStrideSamples === this.#descriptor.rowStrideSamples;
  }

  public replaceFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    if (!this.canReplaceFrame(descriptor)) throw new Error('GPU_FRAME_EXTENT_CHANGED: executor resources must be rebuilt');
    this.#descriptor = descriptor;
    this.invalidateBindings();
    this.#audit = new TransferAudit();
    this.uploadFrame(raw, rawByteOffset, descriptor);
  }

  public prepare(identity: ExecutionIdentity): void {
    this.#gpu.device.queue.writeBuffer(this.#uniforms, 0, packFusedUniforms(this.#descriptor, identity.frameIndex, this.#demosaicParameterValues, this.#quantizationConfig));
    if (this.#demMethod === '00') {
      this.#fullPipeline ??= this.createPipeline(compileFusedNormalShader('00'), 'normal_fused_main');
      this.#fullBindGroup = this.#gpu.device.createBindGroup({
        layout: this.#fullPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.#rawTexture.createView() },
          { binding: 1, resource: this.#blcTexture.createView() },
          { binding: 2, resource: this.#wbcTexture.createView() },
          { binding: 3, resource: this.#demTexture.createView() },
          { binding: 4, resource: this.#colorTexture.createView() },
          { binding: 5, resource: this.#gammaTexture.createView() },
          { binding: 6, resource: this.#outputTexture.createView() },
          { binding: 7, resource: { buffer: this.#uniforms } },
        ],
      });
      return;
    }
    this.prepareSegmented();
  }

  public async execute(_phase: FramePhase, identity: ExecutionIdentity): Promise<readonly PreviewDescriptor[]> {
    if (this.#demMethod === '00' && (this.#fullPipeline === null || this.#fullBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: fused pipeline was not prepared');
    if (this.#demMethod !== '00' && (this.#prePipeline === null || this.#demPipeline === null || this.#demQuantizePipeline === null || this.#postPipeline === null || this.#preBindGroup === null || this.#demBindGroup === null || this.#demQuantizeBindGroup === null || this.#postBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: segmented pipeline was not prepared');
    resizePreviewCanvas(this.#gpu.canvas, this.#descriptor);
    const encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-fused-frame' });
    if (this.#demMethod === '00') {
      this.encodeCompute(encoder, this.#fullPipeline!, this.#fullBindGroup!, 'normal-fused-compute');
    } else {
      this.encodeCompute(encoder, this.#prePipeline!, this.#preBindGroup!, 'normal-fused-pre');
      this.encodeCompute(encoder, this.#demPipeline!, this.#demBindGroup!, 'normal-fused-dem');
      this.encodeCompute(encoder, this.#demQuantizePipeline!, this.#demQuantizeBindGroup!, 'normal-fused-dem-quantize');
      this.encodeCompute(encoder, this.#postPipeline!, this.#postBindGroup!, 'normal-fused-post');
    }
    const previews = normalManifest.preview_outputs.map((capability) => ({
      nodeId: capability.node_id,
      portId: capability.port_id,
      frameIndex: identity.frameIndex,
      runRevision: identity.runRevision,
      methodRevision: identity.methodRevision,
      gpuGeneration: identity.gpuGeneration,
      width: this.#descriptor.width,
      height: this.#descriptor.height,
      format: capability.format,
      domain: capability.domain,
      range: capability.range,
      channelLayout: capability.channel_layout,
      presentation: capability.presentation,
    }));
    const finalView = this.previewView(previews[0]);
    if (finalView === null) throw new Error('PREVIEW_UNAVAILABLE: normal graph has no final preview output');
    this.#presenter.encode(encoder, finalView);
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#gpu.device.queue.onSubmittedWorkDone();
    this.#committedPreviews = previews;
    return previews;
  }

  public async present(nodeA: string, nodeB: string | null, curtain: number): Promise<void> {
    const a = this.previewView(this.#committedPreviews.find((preview) => preview.nodeId === nodeA));
    const b = nodeB === null ? null : this.previewView(this.#committedPreviews.find((preview) => preview.nodeId === nodeB));
    if (a === null || (nodeB !== null && b === null)) throw new Error('PREVIEW_UNAVAILABLE: requested GPU view is not committed');
    await this.#presenter.render(a, b, curtain);
  }

  public reset(): void {
    this.#committedPreviews = [];
    this.#presenter.clear();
    this.invalidateBindings();
  }

  public dispose(): void {
    this.#rawTexture.destroy();
    this.#blcTexture.destroy();
    this.#wbcTexture.destroy();
    this.#demTexture.destroy();
    this.#demIntermediateTexture.destroy();
    this.#colorTexture.destroy();
    this.#gammaTexture.destroy();
    this.#outputTexture.destroy();
    this.#uniforms.destroy();
    this.#demUniforms?.destroy();
    this.#sampleBuffer?.destroy();
  }

  public async sample(nodeId: string, x: number, y: number): Promise<readonly number[]> {
    const descriptor = this.#committedPreviews.find((preview) => preview.nodeId === nodeId);
    const view = this.previewView(descriptor);
    if (view === null || x < 0 || y < 0 || x >= view.descriptor.width || y >= view.descriptor.height) throw new Error('PREVIEW_SAMPLE_UNAVAILABLE: requested sample is outside the committed output');
    this.#sampleBuffer ??= this.#gpu.device.createBuffer({ label: 'preview-sample', size: 256, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const encoder = this.#gpu.device.createCommandEncoder({ label: 'preview-sample-copy' });
    encoder.copyTextureToBuffer({ texture: view.texture, origin: { x, y } }, { buffer: this.#sampleBuffer, bytesPerRow: 256, rowsPerImage: 1 }, { width: 1, height: 1, depthOrArrayLayers: 1 });
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#sampleBuffer.mapAsync(GPUMapMode.READ);
    const mapped = this.#sampleBuffer.getMappedRange();
    const values = view.descriptor.format === 'r16_uint' ? [new Uint16Array(mapped, 0, 1)[0] ?? 0]
      : HALF_FLOAT_PREVIEW_NODES.has(view.descriptor.nodeId) ? Array.from(new Uint16Array(mapped, 0, 4), decodeFloat16)
        : Array.from(new Float32Array(mapped, 0, view.descriptor.format === 'r32_float' ? 1 : 4));
    this.#sampleBuffer.unmap();
    return values;
  }


  public transferAudit(): TransferAuditSnapshot { return this.#audit.snapshot(); }

  public setMethod(nodeId: string, method: string): void {
    const node = normalManifest.nodes.find((candidate) => candidate.id === nodeId);
    if (node === undefined || !node.methods.some((candidate) => candidate.method === method)) throw new Error(`METHOD_INVALID: ${nodeId}.${method}`);
    if (nodeId === 'dem') {
      if (!DEM_METHODS.includes(method as DemMethod)) throw new Error(`METHOD_INVALID: dem.${method}`);
      this.#demMethod = method as DemMethod;
      this.#demQuantizePipeline = null;
      this.#fullPipeline = null;
      this.#prePipeline = null;
      this.#demPipeline = null;
      this.#postPipeline = null;
      this.invalidateBindings();
    }
  }

  public setQuantizationConfig(config: QuantizationConfig): void {
    this.#quantizationConfig = config;
    this.invalidateBindings();
  }

  public setParameter(parameter: string, value: number): void {
    if (!(parameter in this.#demosaicParameterValues) || !Number.isFinite(value)) throw new Error(`PARAMETER_INVALID: ${parameter}`);
    this.#demosaicParameterValues[parameter as 'vng_threshold' | 'ahd_l_threshold' | 'ahd_c_threshold_sq'] = value;
    this.invalidateBindings();
  }

  private prepareSegmented(): void {
    if (this.#demUniforms === null) this.#demUniforms = this.#gpu.device.createBuffer({ label: 'normal-fused-dem-params', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const segmented = compileSegmentedNormalShaders(this.#demMethod);
    this.#prePipeline ??= this.createPipeline(segmented.pre, 'pre_demosaic_main');
    const method = this.#demMethod as Exclude<DemMethod, '00'>;
    this.#demPipeline ??= this.createPipeline(segmented.dem, DEM_ENTRY_POINTS[method]);
    this.#demQuantizePipeline ??= this.createPipeline(segmented.quantize, 'quantize_dem_main');
    this.#postPipeline ??= this.createPipeline(segmented.post, 'postprocess_main');
    const demParams = new ArrayBuffer(32);
    const view = new DataView(demParams);
    const cfa = { rggb: [0, 1, 1, 2], grbg: [1, 0, 2, 1], gbrg: [1, 2, 0, 1], bggr: [2, 1, 1, 0] }[this.#descriptor.cfa];
    cfa.forEach((channel, index) => view.setUint32(index * 4, channel, true));
    view.setFloat32(16, this.#demosaicParameterValues.vng_threshold, true);
    view.setFloat32(20, this.#demosaicParameterValues.ahd_l_threshold, true);
    view.setFloat32(24, this.#demosaicParameterValues.ahd_c_threshold_sq, true);
    this.#gpu.device.queue.writeBuffer(this.#demUniforms, 0, demParams);
    this.#preBindGroup = this.#gpu.device.createBindGroup({ layout: this.#prePipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: this.#rawTexture.createView() },
      { binding: 1, resource: this.#blcTexture.createView() },
      { binding: 2, resource: this.#wbcTexture.createView() },
      { binding: 3, resource: { buffer: this.#uniforms } },
    ] });
    this.#demBindGroup = this.#gpu.device.createBindGroup({ layout: this.#demPipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: { buffer: this.#demUniforms } },
      { binding: 1, resource: this.#wbcTexture.createView() },
      { binding: 2, resource: this.#demIntermediateTexture.createView() },
    ] });
    this.#demQuantizeBindGroup = this.#gpu.device.createBindGroup({ layout: this.#demQuantizePipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: this.#demIntermediateTexture.createView() },
      { binding: 1, resource: this.#demTexture.createView() },
      { binding: 2, resource: { buffer: this.#uniforms } },
    ] });
    this.#postBindGroup = this.#gpu.device.createBindGroup({ layout: this.#postPipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: this.#demTexture.createView() },
      { binding: 1, resource: this.#colorTexture.createView() },
      { binding: 2, resource: this.#gammaTexture.createView() },
      { binding: 3, resource: this.#outputTexture.createView() },
      { binding: 4, resource: { buffer: this.#uniforms } },
    ] });
  }

  private previewView(descriptor: PreviewDescriptor | undefined): PreviewView | null {
    if (descriptor === undefined) return null;
    let cursor = descriptor.nodeId;
    for (let depth = 0; depth < normalManifest.nodes.length; depth += 1) {
      const texture = this.#previewTextures[cursor];
      if (texture !== undefined) return { texture, descriptor };
      const incoming = normalManifest.edges.find((edge) => edge.to.node_id === cursor);
      if (incoming === undefined) return null;
      cursor = incoming.from.node_id;
    }
    return null;
  }

  private uploadFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    const expected = descriptor.rowStrideSamples * descriptor.height;
    if (rawByteOffset % 2 !== 0 || rawByteOffset < 0 || rawByteOffset + expected * 2 !== raw.byteLength) throw new Error(`INPUT_INVALID: expected ${expected} RAW samples`);
    this.#gpu.device.queue.writeTexture({ texture: this.#rawTexture }, new Uint16Array(raw, rawByteOffset, expected), { bytesPerRow: descriptor.rowStrideSamples * 2, rowsPerImage: descriptor.height }, { width: descriptor.width, height: descriptor.height, depthOrArrayLayers: 1 });
    this.#audit.recordRawUpload(expected * 2);
  }

  private createTexture(label: string, format: GPUTextureFormat, usage: GPUTextureUsageFlags): GPUTexture {
    return this.#gpu.device.createTexture({ label, size: [this.#descriptor.width, this.#descriptor.height, 1], format, usage });
  }

  private createPipeline(source: string, entryPoint: string): GPUComputePipeline {
    return this.#gpu.device.createComputePipeline({ label: entryPoint, layout: 'auto', compute: { module: this.#gpu.device.createShaderModule({ code: source }), entryPoint } });
  }

  private encodeCompute(encoder: GPUCommandEncoder, pipeline: GPUComputePipeline, bindGroup: GPUBindGroup, label: string): void {
    const pass = encoder.beginComputePass({ label });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8));
    pass.end();
  }

  private invalidateBindings(): void {
    this.#fullBindGroup = null;
    this.#demQuantizeBindGroup = null;
    this.#preBindGroup = null;
    this.#demBindGroup = null;
    this.#postBindGroup = null;
  }
}
function decodeFloat16(bits: number): number {
  const sign = (bits & 0x8000) === 0 ? 1 : -1;
  const exponent = (bits >>> 10) & 0x1f;
  const fraction = bits & 0x3ff;
  if (exponent === 0) return sign * 2 ** -14 * (fraction / 1024);
  if (exponent === 0x1f) return fraction === 0 ? sign * Infinity : Number.NaN;
  return sign * 2 ** (exponent - 15) * (1 + fraction / 1024);
}
