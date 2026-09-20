import { normalManifest } from '../generated/normal_manifest.generated.js';
import { lcstPipeline } from '../generated/lcst_pipeline.generated.js';
import type { DrcIqParameters, FrameBeginPackets, FrameConsumerPackets, FrameMode, FramePhase, PreviewDescriptor, RawFrameDescriptor, TransferAuditSnapshot } from '../contracts.js';
import type { ExecutionIdentity } from '../runtime-controller.js';
import { resizePreviewCanvas } from '../preview-state.js';
import type { GpuContext } from './device.js';
import { validateGraphBypassConfig, type GraphBypassConfig } from './bypass.js';
import { blcPipelineWgsl } from '../generated/blc_pipeline.generated.js';
import { lscPipelineWgsl } from '../generated/lsc_pipeline.generated.js';
import { tintlessPipelineWgsl } from '../generated/tintless_pipeline.generated.js';
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
const HALF_FLOAT_PREVIEW_NODES = new Set(['dem', 'color_reproduce', 'rgb2yuv']);
export interface StagedFramePacketProvider {
  begin(identity: ExecutionIdentity, mode: FrameMode): FrameBeginPackets;
  prepareConsumers(payload: Uint8Array | null, coldStart: boolean): FrameConsumerPackets;
  stageLcstStatistics(payload: Uint8Array): void;
}
export class NormalGpuExecutor {
  readonly #gpu: GpuContext;
  readonly #presenter: GpuPreviewPresenter;
  readonly #rawTexture: GPUTexture;
  readonly #blcTexture: GPUTexture;
  readonly #tintlessTexture: GPUTexture;
  readonly #lscTexture: GPUTexture;
  readonly #drcTexture: GPUTexture;
  readonly #wbcTexture: GPUTexture;
  readonly #demTexture: GPUTexture;
  readonly #demIntermediateTexture: GPUTexture;
  readonly #colorTexture: GPUTexture;
  readonly #outputTexture: GPUTexture;
  #uniforms: GPUBuffer | null = null;
  readonly #crHsLut: GPUBuffer;
  readonly #previewTextures: Readonly<Record<string, GPUTexture>>;
  #committedPreviews: readonly PreviewDescriptor[] = [];
  #demUniforms: GPUBuffer | null = null;
  readonly #drc: WebDrcExecutor;
  readonly #packetProvider: StagedFramePacketProvider;
  readonly #mode: FrameMode;
  #lcstUniform: GPUBuffer | null = null;
  readonly #lcstAverage: GPUBuffer;
  readonly #lcstHistogram: GPUBuffer;
  readonly #lcstReadback: GPUBuffer;
  #consumerPackets: FrameConsumerPackets | null = null;
  #preparedIdentity: ExecutionIdentity | null = null;
  #lcstStaged = false;
  #audit = new TransferAudit();
  #descriptor: RawFrameDescriptor;
  #demMethod: DemMethod = '00';
  #drcMethod: DrcMethod = '00';
  #drcBypassed = false;
  #lscBypassed = false;
  #tintlessBypassed = false;
  #lscActive = false;
  #moduleShaders: ModuleShaderRuntime | null = null;
  #blcUniform: GPUBuffer | null = null;
  #tintlessUniform: GPUBuffer | null = null;
  #tintlessMesh: GPUBuffer | null = null;
  #lscUniform: GPUBuffer | null = null;
  #lscMeshHeaders: GPUBuffer | null = null;
  #lscMeshEntries: GPUBuffer | null = null;
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
  #sampleBuffer: GPUBuffer | null = null;
  public constructor(gpu: GpuContext, raw: ArrayBuffer, rawByteOffset: number, _generation: number, descriptor: RawFrameDescriptor, mode: FrameMode, packetProvider: StagedFramePacketProvider) {
    this.#gpu = gpu;
    this.#descriptor = descriptor;
    this.#mode = mode;
    this.#packetProvider = packetProvider;
    this.#presenter = new GpuPreviewPresenter(gpu.context, gpu.device, gpu.canvasFormat);
    const previewUsage = GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC;
    this.#rawTexture = this.createTexture('normal-raw-source', 'r16uint', GPUTextureUsage.COPY_DST | previewUsage);
    this.#blcTexture = this.createTexture('normal-blc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#tintlessTexture = this.createTexture('normal-tintless', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#lscTexture = this.createTexture('normal-lsc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#drcTexture = this.createTexture('normal-drc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#wbcTexture = this.createTexture('normal-wbc', 'r32float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demTexture = this.createTexture('normal-dem', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#demIntermediateTexture = this.createTexture('normal-dem-intermediate', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING);
    this.#colorTexture = this.createTexture('normal-color', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#outputTexture = this.createTexture('normal-yuv', 'rgba16float', GPUTextureUsage.STORAGE_BINDING | previewUsage);
    this.#previewTextures = { raw_source: this.#rawTexture, blc: this.#blcTexture, tintless: this.#tintlessTexture, lsc: this.#lscTexture, drc: this.#drcTexture, wbc: this.#wbcTexture, dem: this.#demTexture, color_reproduce: this.#colorTexture, rgb2yuv: this.#outputTexture };
    const hsLut = descriptor.colorReproduce?.hsLut ?? [];
    this.#crHsLut = gpu.device.createBuffer({ label: 'cr-hs-lut', size: Math.max(hsLut.length * 4, 16), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.#lcstAverage = gpu.device.createBuffer({ label: 'lcst-average-payload', size: lcstPipeline.averageBytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
    this.#lcstHistogram = gpu.device.createBuffer({ label: 'lcst-histogram-payload', size: lcstPipeline.histogramBytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
    this.#lcstReadback = gpu.device.createBuffer({ label: 'lcst-payload-readback', size: lcstPipeline.payloadBytes, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
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
    const begin = this.#packetProvider.begin(identity, this.#mode);
    this.#preparedIdentity = identity;
    this.#lcstStaged = false;
    this.#moduleShaders ??= new ModuleShaderRuntime(this.#gpu.device);
    this.#lcstUniform ??= this.#gpu.device.createBuffer({ label: 'lcst-params', size: Math.max(begin.lcstUniform.byteLength, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#lcstUniform, 0, begin.lcstUniform);
    this.#blcUniform ??= this.#gpu.device.createBuffer({ label: 'normal-blc-params', size: Math.max(begin.blcUniform.byteLength, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#blcUniform, 0, begin.blcUniform);
    if (this.#mode === 'sequence') this.installConsumerPackets(this.#packetProvider.prepareConsumers(null, identity.frameIndex === 0));
  }

  private installConsumerPackets(packets: FrameConsumerPackets): void {
    this.#consumerPackets = packets;
    this.#lscActive = packets.lscActive;
    this.#uniforms ??= this.#gpu.device.createBuffer({ label: 'normal-fused-params', size: packets.fusedUniform.byteLength, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#uniforms, 0, packets.fusedUniform);
    this.#tintlessUniform ??= this.#gpu.device.createBuffer({ label: 'normal-tintless-params', size: Math.max(packets.tintlessUniform.byteLength, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const tintlessMeshBytes = Math.max(packets.tintlessMesh.byteLength, 16);
    if (this.#tintlessMesh === null || this.#tintlessMesh.size < tintlessMeshBytes) {
      this.#tintlessMesh?.destroy();
      this.#tintlessMesh = this.#gpu.device.createBuffer({ label: 'normal-tintless-mesh', size: tintlessMeshBytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    }
    this.#lscUniform ??= this.#gpu.device.createBuffer({ label: 'normal-lsc-params', size: Math.max(packets.lscUniform.byteLength, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const lscMeshHeadersBytes = Math.max(packets.lscMeshHeaders.byteLength, 32);
    if (this.#lscMeshHeaders === null || this.#lscMeshHeaders.size < lscMeshHeadersBytes) {
      this.#lscMeshHeaders?.destroy();
      this.#lscMeshHeaders = this.#gpu.device.createBuffer({ label: 'normal-lsc-mesh-headers', size: lscMeshHeadersBytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    }
    const lscMeshEntriesBytes = Math.max(packets.lscMeshEntries.byteLength, 16);
    if (this.#lscMeshEntries === null || this.#lscMeshEntries.size < lscMeshEntriesBytes) {
      this.#lscMeshEntries?.destroy();
      this.#lscMeshEntries = this.#gpu.device.createBuffer({ label: 'normal-lsc-mesh-entries', size: lscMeshEntriesBytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    }
    this.#wbcUniform ??= this.#gpu.device.createBuffer({ label: 'normal-wbc-params', size: Math.max(packets.wbcUniform.byteLength, 16), usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.#gpu.device.queue.writeBuffer(this.#tintlessUniform, 0, packets.tintlessUniform);
    this.#gpu.device.queue.writeBuffer(this.#tintlessMesh, 0, packets.tintlessMesh);
    this.#gpu.device.queue.writeBuffer(this.#lscUniform, 0, packets.lscUniform);
    this.#gpu.device.queue.writeBuffer(this.#lscMeshHeaders, 0, packets.lscMeshHeaders);
    this.#gpu.device.queue.writeBuffer(this.#lscMeshEntries, 0, packets.lscMeshEntries);
    this.#gpu.device.queue.writeBuffer(this.#wbcUniform, 0, packets.wbcUniform);
    if (!this.#drcBypassed) {
      this.#drc.setMethod(this.#drcMethod);
      this.#drc.preparePackets(packets.drcUniform, packets.drcGlobalLut, packets.drcLocalLut, packets.drcModulationLuts);
    }
    if (this.#demMethod === '00') {
      this.#fullPipeline ??= this.createPipeline(compileFusedNormalShader(), 'normal_fused_main');
      this.#fullBindGroup = this.#gpu.device.createBindGroup({
        layout: this.#fullPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.#drcTexture.createView() },
          { binding: 1, resource: this.#wbcTexture.createView() },
          { binding: 2, resource: this.#demTexture.createView() },
          { binding: 3, resource: this.#colorTexture.createView() },
          { binding: 4, resource: this.#outputTexture.createView() },
          { binding: 5, resource: { buffer: this.#uniforms } },
          { binding: 6, resource: { buffer: this.#crHsLut } },
        ],
      });
      return;
    }
    this.prepareSegmented(packets.demUniform);
  }

  public async execute(_phase: FramePhase, identity: ExecutionIdentity): Promise<readonly PreviewDescriptor[]> {
    if (this.#moduleShaders === null || this.#blcUniform === null) throw new Error('FUSED_GRAPH_INVALID: BLC pipeline was not prepared');
    if (this.#mode === 'sequence' && this.#consumerPackets === null) throw new Error('FUSED_GRAPH_INVALID: sequence consumers were not prepared');
    resizePreviewCanvas(this.#gpu.canvas, this.#descriptor);
    let encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-fused-frame' });
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
    if (this.#lcstUniform === null) throw new Error('FUSED_GRAPH_INVALID: LCST pipeline was not prepared');
    this.#moduleShaders.encode(encoder, {
      label: 'normal-lcst-average', source: lcstPipeline.wgsl, entryPoint: lcstPipeline.stages[0].entryPoint,
      bindings: [
        { binding: 0, resource: { texture: this.#blcTexture } },
        { binding: 1, resource: { buffer: this.#lcstUniform } },
        { binding: 2, resource: { buffer: this.#lcstAverage } },
      ], workgroups: lcstPipeline.averageDispatch.slice(0, 2) as [number, number],
    });
    this.#moduleShaders.encode(encoder, {
      label: 'normal-lcst-histogram', source: lcstPipeline.wgsl, entryPoint: lcstPipeline.stages[1].entryPoint,
      bindings: [
        { binding: 0, resource: { texture: this.#blcTexture } },
        { binding: 1, resource: { buffer: this.#lcstUniform } },
        { binding: 3, resource: { buffer: this.#lcstHistogram } },
      ], workgroups: lcstPipeline.histogramDispatch.slice(0, 2) as [number, number],
    });
    encoder.copyBufferToBuffer(this.#lcstAverage, 0, this.#lcstReadback, 0, lcstPipeline.averageBytes);
    this.#audit.recordGpuCopy(lcstPipeline.averageBytes, 'lcst-average-readback');
    encoder.copyBufferToBuffer(this.#lcstHistogram, 0, this.#lcstReadback, lcstPipeline.averageBytes, lcstPipeline.histogramBytes);
    this.#audit.recordGpuCopy(lcstPipeline.histogramBytes, 'lcst-histogram-readback');
    if (this.#mode === 'single' && !this.#lcstStaged) {
      this.#gpu.device.queue.submit([encoder.finish()]);
      await this.#gpu.device.queue.onSubmittedWorkDone();
      const payload = await this.readLcstPayload();
      this.#lcstStaged = true;
      this.installConsumerPackets(this.#packetProvider.prepareConsumers(payload, false));
      encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-fused-downstream' });
    }
    if (this.#demMethod === '00' && (this.#fullPipeline === null || this.#fullBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: fused pipeline was not prepared');
    if (this.#demMethod !== '00' && (this.#prePipeline === null || this.#demPipeline === null || this.#demQuantizePipeline === null || this.#postPipeline === null || this.#preBindGroup === null || this.#demBindGroup === null || this.#demQuantizeBindGroup === null || this.#postBindGroup === null)) throw new Error('FUSED_GRAPH_INVALID: segmented pipeline was not prepared');
    if (this.#consumerPackets === null) throw new Error('FUSED_GRAPH_INVALID: frame consumers were not prepared');


    if (this.#tintlessUniform === null || this.#tintlessMesh === null) throw new Error('FUSED_GRAPH_INVALID: Tintless pipeline was not prepared');
    const tintlessOutput = this.#tintlessBypassed ? this.#blcTexture : this.#tintlessTexture;
    if (!this.#tintlessBypassed) {
      this.#moduleShaders.encode(encoder, {
        label: 'normal-tintless',
        source: tintlessPipelineWgsl,
        entryPoint: 'tintless_main',
        bindings: [
          { binding: 0, resource: { texture: this.#blcTexture } },
          { binding: 1, resource: { texture: this.#tintlessTexture } },
          { binding: 2, resource: { buffer: this.#tintlessUniform } },
          { binding: 3, resource: { buffer: this.#tintlessMesh } },
        ],
        workgroups: [Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8)],
      });
    }
    if (this.#lscUniform === null || this.#lscMeshHeaders === null || this.#lscMeshEntries === null) throw new Error('FUSED_GRAPH_INVALID: LSC pipeline was not prepared');
    const lscExecuted = !this.#lscBypassed && this.#lscActive;
    const lscOutput = lscExecuted ? this.#lscTexture : tintlessOutput;
    if (lscExecuted) {
      this.#moduleShaders.encode(encoder, {
        label: 'normal-lsc',
        source: lscPipelineWgsl,
        entryPoint: 'lsc_main',
        bindings: [
          { binding: 0, resource: { texture: tintlessOutput } },
          { binding: 1, resource: { texture: this.#lscTexture } },
          { binding: 2, resource: { buffer: this.#lscUniform } },
          { binding: 3, resource: { buffer: this.#lscMeshHeaders } },
          { binding: 4, resource: { buffer: this.#lscMeshEntries } },
        ],
        workgroups: [Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8)],
      });
    }
    // Manifest order: WBC precedes DRC (wbc -> drc -> dem). Native and web
    // share the identical dataflow — DRC consumes the white-balanced Bayer,
    // post-DRC passes read the DRC output without re-applying phase gains.
    if (this.#moduleShaders === null || this.#wbcUniform === null) throw new Error('FUSED_GRAPH_INVALID: WBC pipeline was not prepared');
    // wbc00.wgsl declares: binding 0 = input_tex, 1 = output_tex, 2 = uniform.
    this.#moduleShaders.encode(encoder, {
      label: 'normal-wbc',
      source: wbcPipelineWgsl,
      entryPoint: 'wbc_main',
      bindings: [
        { binding: 0, resource: { texture: lscOutput } },
        { binding: 1, resource: { texture: this.#drcBypassed ? this.#drcTexture : this.#wbcTexture } },
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
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#gpu.device.queue.onSubmittedWorkDone();
    if (!this.#lcstStaged) {
      const payload = await this.readLcstPayload();
      this.#lcstStaged = true;
      if (this.#mode === 'sequence') this.#packetProvider.stageLcstStatistics(payload);
      else this.#consumerPackets = this.#packetProvider.prepareConsumers(payload, false);
    }
    return previews;
  }
  private async readLcstPayload(): Promise<Uint8Array> {
    this.#audit.recordStatisticsRead(lcstPipeline.payloadBytes, 'LCST fixed statistics payload');
    await this.#lcstReadback.mapAsync(GPUMapMode.READ);
    const mapped = this.#lcstReadback.getMappedRange();
    const payload = new Uint8Array(lcstPipeline.payloadBytes);
    payload.set(new Uint8Array(mapped, 0, lcstPipeline.payloadBytes));
    this.#lcstReadback.unmap();
    return payload;
  }


  public async commit(previews: readonly PreviewDescriptor[]): Promise<void> {
    const finalView = this.previewView(previews[0]);
    if (finalView === null) throw new Error('PREVIEW_UNAVAILABLE: normal graph has no final preview output');
    const encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-preview-commit' });
    this.#presenter.encode(encoder, finalView);
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#gpu.device.queue.onSubmittedWorkDone();
    this.#committedPreviews = previews;
  }

  public abort(): void {
    this.#consumerPackets = null;
    this.#preparedIdentity = null;
    this.#lcstStaged = false;
    this.#committedPreviews = [];
    this.#presenter.clear();
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
    this.#tintlessTexture.destroy();
    this.#lscTexture.destroy();
    this.#drcTexture.destroy();
    this.#wbcTexture.destroy();
    this.#demTexture.destroy();
    this.#demIntermediateTexture.destroy();
    this.#colorTexture.destroy();
    this.#outputTexture.destroy();
    this.#uniforms?.destroy();
    this.#crHsLut.destroy();
    this.#lcstUniform?.destroy();
    this.#lcstAverage.destroy();
    this.#lcstHistogram.destroy();
    this.#lcstReadback.destroy();
    this.#tintlessUniform?.destroy();
    this.#tintlessMesh?.destroy();
    this.#lscUniform?.destroy();
    this.#lscMeshHeaders?.destroy();
    this.#lscMeshEntries?.destroy();
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
    this.#tintlessBypassed = config.modules.find((module) => module.module_id === 'tintless')?.bypass === true;
    this.#lscBypassed = config.modules.find((module) => module.module_id === 'lsc')?.bypass === true;
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
      { binding: 0, resource: this.#drcTexture.createView() },
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
      { binding: 2, resource: this.#outputTexture.createView() },
      { binding: 3, resource: { buffer: this.#uniforms } },
      { binding: 4, resource: { buffer: this.#crHsLut } },
    ] });
  }

  private previewView(descriptor: PreviewDescriptor | undefined): PreviewView | null {
    if (descriptor === undefined) return null;
    let cursor = descriptor.nodeId;
    for (let depth = 0; depth < normalManifest.nodes.length; depth += 1) {
      const texture = this.#tintlessBypassed && cursor === 'tintless'
        ? this.#blcTexture
        : (this.#lscBypassed || !this.#lscActive) && cursor === 'lsc'
          ? (this.#tintlessBypassed ? this.#blcTexture : this.#tintlessTexture)
          : this.#drcBypassed && cursor === 'drc' ? this.#drcTexture : this.#previewTextures[cursor];
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
    this.#tintlessUniform?.destroy();
    this.#tintlessUniform = null;
    this.#tintlessMesh?.destroy();
    this.#tintlessMesh = null;
    this.#lscUniform?.destroy();
    this.#lscUniform = null;
    this.#lscMeshHeaders?.destroy();
    this.#lscMeshHeaders = null;
    this.#lscMeshEntries?.destroy();
    this.#lscMeshEntries = null;
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
