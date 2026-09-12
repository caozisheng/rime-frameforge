import { normalManifest } from '../generated/normal_manifest.generated.js';
import type { DrcIqParameters, FramePacketProvider, FramePhase, PreviewDescriptor, RawFrameDescriptor, TransferAuditSnapshot } from '../contracts.js';
import type { ExecutionIdentity } from '../runtime-controller.js';
import { resizePreviewCanvas } from '../preview-state.js';
import type { GpuContext } from './device.js';
import { validateGraphBypassConfig, type GraphBypassConfig } from './bypass.js';
import { blcPipelineWgsl } from '../generated/blc_pipeline.generated.js';
import { wbcPipelineWgsl } from '../generated/wbc_pipeline.generated.js';
import { compileFusedNormalShader, compileSegmentedNormalShaders, isSegmentedDemMethod } from './fused-normal-shader.js';
import { ModuleShaderRuntime } from './module-shader.js';
import { validateDrcIqParameters, validateModulationCurves, WebDrcExecutor, type DrcMethod, type DrcModulationCurves } from './drc.js';
import { GpuPreviewPresenter, type PreviewView } from './presenter.js';
import { TransferAudit } from './transfer-audit.js';

const DEM_METHODS = ['00', '01', '02', '03', '04'] as const;
type DemMethod = (typeof DEM_METHODS)[number];
const DEM_ENTRY_POINTS: Record<Exclude<DemMethod, '00'>, string> = {
  '01': 'demosaic_mhc_main',
  '02': 'demosaic_ppg_main',
  '03': 'demosaic_vng_main',
  '04': 'demosaic_ahd_main',
};
const HALF_FLOAT_PREVIEW_NODES = new Set(['dem', 'color_reproduce', 'gamma', 'rgb2yuv']);
export class NormalGpuExecutor {
  readonly #gpu: GpuContext;
  readonly #presenter: GpuPreviewPresenter;
  readonly #rawTexture: GPUTexture;
  readonly #blcTexture: GPUTexture;
  readonly #drcTexture: GPUTexture;
  readonly #wbcTexture: GPUTexture;
  readonly #demTexture: GPUTexture;
  readonly #demIntermediateTexture: GPUTexture;
  readonly #colorTexture: GPUTexture;
  readonly #gammaTexture: GPUTexture;
  readonly #outputTexture: GPUTexture;
  #uniforms: GPUBuffer | null = null;
  readonly #crHsLut: GPUBuffer;
  readonly #previewTextures: Readonly<Record<string, GPUTexture>>;
  readonly #drc: WebDrcExecutor;
  readonly #packetProvider: FramePacketProvider;
  #demUniforms: GPUBuffer | null = null;
  #audit = new TransferAudit();
  #descriptor: RawFrameDescriptor;
  #demMethod: DemMethod = '00';
  #drcMethod: DrcMethod = '00';
  #drcBypassed = false;
  #moduleShaders: ModuleShaderRuntime | null = null;
  #blcUniform: GPUBuffer | null = null;
  #wbcUniform: GPUBuffer | null = null;
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
  public constructor(gpu: GpuContext, raw: ArrayBuffer, rawByteOffset: number, _generation: number, descriptor: RawFrameDescriptor, packetProvider: FramePacketProvider) {
    this.#gpu = gpu;
    this.#descriptor = descriptor;
    this.#packetProvider = packetProvider;
    this.#presenter = new GpuPreviewPresenter(gpu.context, gpu.device, gpu.canvasFormat);
    const previewUsage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC;
    this.#rawTexture = this.createTexture('normal-raw-source', 'r16uint', GPUTextureUsage.COPY_DST | previewUsage);
    this.#blcTexture = this.createTexture('normal-blc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#drcTexture = this.createTexture('normal-drc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#wbcTexture = this.createTexture('normal-wbc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demTexture = this.createTexture('normal-dem', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demIntermediateTexture = this.createTexture('normal-dem-intermediate', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING);
    this.#colorTexture = this.createTexture('normal-color', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#gammaTexture = this.createTexture('normal-gamma', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#outputTexture = this.createTexture('normal-yuv', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#previewTextures = {
      raw_source: this.#rawTexture,
      blc: this.#blcTexture,
      drc: this.#drcTexture,
      wbc: this.#wbcTexture,
      dem: this.#demTexture,
      color_reproduce: this.#colorTexture,
      gamma: this.#gammaTexture,
      rgb2yuv: this.#outputTexture,
    };
    const hsLut = descriptor.colorReproduce?.hsLut ?? [];
    this.#crHsLut = gpu.device.createBuffer({ label: 'cr-hs-lut', size: Math.max(hsLut.length * 4, 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.#drc = new WebDrcExecutor(gpu.device, descriptor.width, descriptor.height);
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
    const packets = this.#packetProvider(identity);
    this.#uniforms ??= this.#gpu.device.createBuffer({ label: 'normal-fused-params', size: packets.fusedUniform.byteLength, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#uniforms, 0, packets.fusedUniform);
    if (packets.colorReproduceHsLut.byteLength > 0) this.#gpu.device.queue.writeBuffer(this.#crHsLut, 0, packets.colorReproduceHsLut);
    if (!this.#drcBypassed) {
      this.#drc.setMethod(this.#drcMethod);
      this.#drc.preparePackets(packets.drcUniform, packets.drcGlobalLut, packets.drcLocalLut, packets.drcModulationLuts);
    }
    this.#moduleShaders ??= new ModuleShaderRuntime(this.#gpu.device);
    this.#blcUniform ??= this.#gpu.device.createBuffer({ label: 'blc-scalars', size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#blcUniform, 0, packets.blcUniform);
    this.#wbcUniform ??= this.#gpu.device.createBuffer({ label: 'wbc-scalars', size: 48, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#wbcUniform, 0, packets.wbcUniform);
    if (this.#demMethod === '00') {
      this.#fullPipeline ??= this.createPipeline(compileFusedNormalShader(), 'normal_fused_main');
      this.#fullBindGroup = this.#gpu.device.createBindGroup({
        layout: this.#fullPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.drcInputTexture().createView() },
          { binding: 1, resource: this.#wbcTexture.createView() },
          { binding: 2, resource: this.#demTexture.createView() },
          { binding: 3, resource: this.#colorTexture.createView() },
          { binding: 4, resource: this.#gammaTexture.createView() },
          { binding: 5, resource: this.#outputTexture.createView() },
          { binding: 6, resource: { buffer: this.#uniforms } },
          { binding: 7, resource: { buffer: this.#crHsLut } },
        ],
      });
      return;
    }
    this.prepareSegmented(packets.demUniform);
  }

  public async execute(_phase: FramePhase, identity: ExecutionIdentity): Promise<readonly PreviewDescriptor[]> {
    if (this.#moduleShaders === null || this.#blcUniform === null) throw new Error('FUSED_GRAPH_INVALID: BLC pipeline was not prepared');
    if (this.#demMethod === '00' && (this.#fullPipeline === null || this.#fullBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: fused pipeline was not prepared');
    if (this.#demMethod !== '00' && (this.#prePipeline === null || this.#demPipeline === null || this.#demQuantizePipeline === null || this.#postPipeline === null || this.#preBindGroup === null || this.#demBindGroup === null || this.#demQuantizeBindGroup === null || this.#postBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: segmented pipeline was not prepared');
    resizePreviewCanvas(this.#gpu.canvas, this.#descriptor);
    const encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-fused-frame' });
    // blc00.wgsl declares: binding 0 = uniform, 1 = input_tex, 2 = output_tex.
    this.#moduleShaders.encode(encoder, {
      label: 'normal-blc',
      source: blcPipelineWgsl,
      entryPoint: 'blc_main',
      bindings: [
        { binding: 0, resource: { buffer: this.#blcUniform } },
        { binding: 1, resource: { texture: this.#rawTexture } },
        { binding: 2, resource: { texture: this.#blcTexture } },
      ],
      workgroups: [Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8)],
    });
    // Manifest order: WBC precedes DRC (wbc -> cac -> drc). Native and web
    // share the identical dataflow — DRC consumes the white-balanced Bayer,
    // post-DRC passes read the DRC output without re-applying phase gains.
    if (this.#moduleShaders === null || this.#wbcUniform === null) throw new Error('FUSED_GRAPH_INVALID: WBC pipeline was not prepared');
    // wbc00.wgsl declares: binding 0 = input_tex, 1 = output_tex, 2 = uniform.
    this.#moduleShaders.encode(encoder, {
      label: 'normal-wbc',
      source: wbcPipelineWgsl,
      entryPoint: 'wbc_main',
      bindings: [
        { binding: 0, resource: { texture: this.#blcTexture } },
        { binding: 1, resource: { texture: this.#wbcTexture } },
        { binding: 2, resource: { buffer: this.#wbcUniform } },
      ],
      workgroups: [Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8)],
    });
    if (!this.#drcBypassed) this.#drc.encode(encoder, this.#wbcTexture, this.#drcTexture);

    if (this.#demMethod === '00') {
      this.encodeCompute(encoder, this.#fullPipeline!, this.#fullBindGroup!, 'normal-post-drc-fused');
    } else {
      this.encodeCompute(encoder, this.#prePipeline!, this.#preBindGroup!, 'normal-post-drc-pre-demosaic');
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
    this.#drcTexture.destroy();
    this.#wbcTexture.destroy();
    this.#demTexture.destroy();
    this.#demIntermediateTexture.destroy();
    this.#colorTexture.destroy();
    this.#gammaTexture.destroy();
    this.#outputTexture.destroy();
    this.#uniforms?.destroy();
    this.#crHsLut.destroy();
    this.#demUniforms?.destroy();
    this.#sampleBuffer?.destroy();
    this.#drc.dispose();
    this.#moduleShaders?.dispose();
    this.#moduleShaders = null;
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

  public setBypassConfig(config: GraphBypassConfig): void {
    validateGraphBypassConfig(config);
    this.#drcBypassed = config.modules.find((module) => module.module_id === 'drc')?.bypass === true;
    this.invalidateBindings();
  }

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
    if (nodeId === 'drc') {
      if (method !== '00' && method !== '01') throw new Error(`METHOD_INVALID: drc.${method}`);
      this.#drcMethod = method;
      this.invalidateBindings();
    }
  }


  public setParameter(nodeId: string, parameter: string, value: number): void {
    const node = normalManifest.nodes.find((candidate) => candidate.id === nodeId);
    const ownsParameter = node?.methods.some((method) => (method.parameters as readonly string[]).includes(parameter)) === true;
    if (!ownsParameter || !Number.isFinite(value)) throw new Error(`PARAMETER_INVALID: ${nodeId}.${parameter}`);
    this.invalidateBindings();
  }

  public setDrcIqParameters(parameters: DrcIqParameters, curves?: DrcModulationCurves): void {
    validateDrcIqParameters(parameters);
    if (curves !== undefined) validateModulationCurves(curves);
    this.invalidateBindings();
  }

  public setLut(parameter: string, _values: readonly number[]): void {
    if (parameter !== 'gamma_lut') throw new Error(`PARAMETER_INVALID: ${parameter}`);
    this.invalidateBindings();
  }

  private prepareSegmented(demUniform: Uint8Array<ArrayBuffer>): void {
    if (this.#uniforms === null) throw new Error('FUSED_GRAPH_INVALID: fused parameters were not prepared');
    if (this.#demUniforms === null) this.#demUniforms = this.#gpu.device.createBuffer({ label: 'normal-fused-dem-params', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    if (!isSegmentedDemMethod(this.#demMethod)) throw new Error(`FUSED_GRAPH_SEGMENT_INVALID: unknown DEM method ${this.#demMethod}`);
    const segmented = compileSegmentedNormalShaders(this.#demMethod);
    this.#prePipeline ??= this.createPipeline(segmented.pre, 'pre_demosaic_main');
    const method = this.#demMethod as Exclude<DemMethod, '00'>;
    this.#demPipeline ??= this.createPipeline(segmented.dem, DEM_ENTRY_POINTS[method]);
    this.#demQuantizePipeline ??= this.createPipeline(segmented.quantize, 'quantize_dem_main');
    this.#postPipeline ??= this.createPipeline(segmented.post, 'postprocess_main');
    this.#gpu.device.queue.writeBuffer(this.#demUniforms, 0, demUniform);
    this.#preBindGroup = this.#gpu.device.createBindGroup({ layout: this.#prePipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: this.drcInputTexture().createView() },
      { binding: 1, resource: this.#wbcTexture.createView() },
      { binding: 2, resource: { buffer: this.#uniforms } },
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
      { binding: 5, resource: { buffer: this.#crHsLut } },
    ] });
  }

  private previewView(descriptor: PreviewDescriptor | undefined): PreviewView | null {
    if (descriptor === undefined) return null;
    let cursor = descriptor.nodeId;
    for (let depth = 0; depth < normalManifest.nodes.length; depth += 1) {
      const texture = this.#drcBypassed && cursor === 'drc' ? this.#wbcTexture : this.#previewTextures[cursor];
      if (texture !== undefined) return { texture, descriptor };
      const incoming = normalManifest.edges.find((edge) => edge.to.node_id === cursor);
      if (incoming === undefined) return null;
      cursor = incoming.from.node_id;
    }
    return null;
  }

  private drcInputTexture(): GPUTexture {
    // Input to the post-DRC passes (graph: wbc -> cac -> drc -> dem -> ...).
    // With DRC bypassed they consume the WBC output directly.
    return this.#drcBypassed ? this.#wbcTexture : this.#drcTexture;
  }

  private uploadFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    const expected = descriptor.rowStrideSamples * descriptor.height;
    if (rawByteOffset % 2 !== 0 || rawByteOffset < 0 || rawByteOffset + expected * 2 !== raw.byteLength) throw new Error(`INPUT_INVALID: expected ${expected} RAW samples`);
    this.#gpu.device.queue.writeTexture({ texture: this.#rawTexture }, new Uint16Array(raw, rawByteOffset, expected), { bytesPerRow: descriptor.rowStrideSamples * 2, rowsPerImage: descriptor.height }, { width: descriptor.width, height: descriptor.height, depthOrArrayLayers: 1 });
    this.#audit.recordRawUpload(expected * 2);
    const crAssets = descriptor.colorReproduce;
    const hsLut = crAssets !== undefined && crAssets !== null && crAssets.hsEnable ? crAssets.hsLut : undefined;
    if (hsLut !== undefined && hsLut !== null && hsLut.length > 0) {
      this.#gpu.device.queue.writeBuffer(this.#crHsLut, 0, new Float32Array(hsLut));
    }
  }

  private createTexture(label: string, format: GPUTextureFormat, usage: GPUTextureUsageFlags): GPUTexture {
    return this.#gpu.device.createTexture({ label, size: [this.#descriptor.width, this.#descriptor.height, 1], format, usage });
  }

  private createPipeline(source: string, entryPoint: string): GPUComputePipeline {
    return this.#gpu.device.createComputePipeline({ label: entryPoint, layout: 'auto', compute: { module: this.#gpu.device.createShaderModule({ code: source }), entryPoint } });
  }

  private encodeCompute(encoder: GPUCommandEncoder, pipeline: GPUComputePipeline, bindGroup: GPUBindGroup, label: string, workgroupsX?: number, workgroupsY?: number): void {
    const pass = encoder.beginComputePass({ label });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    if (workgroupsX !== undefined && workgroupsY !== undefined) {
      pass.dispatchWorkgroups(workgroupsX, workgroupsY);
    } else {
      pass.dispatchWorkgroups(Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8));
    }
    pass.end();
  }

  private invalidateBindings(): void {
    this.#uniforms?.destroy();
    this.#uniforms = null;
    this.#blcUniform?.destroy();
    this.#blcUniform = null;
    this.#wbcUniform?.destroy();
    this.#wbcUniform = null;
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
