import { normalManifest } from '../generated/normal_manifest.generated.js';
import type { FramePhase, PreviewDescriptor, RawFrameDescriptor, TransferAuditSnapshot } from '../contracts.js';
import type { ExecutionIdentity } from '../runtime-controller.js';
import type { GpuContext } from './device.js';
import { compileFusedNormalShader, compileSegmentedNormalShaders } from './fused-normal-shader.js';
import { FUSED_UNIFORM_BYTES, packFusedUniforms } from './fused-uniforms.js';
import { GpuPreviewPresenter } from './presenter.js';
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

export class NormalGpuExecutor {
  readonly #gpu: GpuContext;
  readonly #presenter: GpuPreviewPresenter;
  readonly #rawTexture: GPUTexture;
  readonly #outputTexture: GPUTexture;
  readonly #uniforms: GPUBuffer;
  #preTexture: GPUTexture | null = null;
  #demTexture: GPUTexture | null = null;
  #demUniforms: GPUBuffer | null = null;
  #audit = new TransferAudit();
  #descriptor: RawFrameDescriptor;
  #quantizationConfig: QuantizationConfig;
  #demMethod: DemMethod = '00';
  #fullPipeline: GPUComputePipeline | null = null;
  #prePipeline: GPUComputePipeline | null = null;
  #demPipeline: GPUComputePipeline | null = null;
  #postPipeline: GPUComputePipeline | null = null;
  #fullBindGroup: GPUBindGroup | null = null;
  #preBindGroup: GPUBindGroup | null = null;
  #demBindGroup: GPUBindGroup | null = null;
  #postBindGroup: GPUBindGroup | null = null;
  #demosaicParameterValues = {
    vng_threshold: 1.5,
    ahd_l_threshold: 2.0,
    ahd_c_threshold_sq: 4.0,
  };

  public constructor(
    gpu: GpuContext,
    raw: ArrayBuffer,
    rawByteOffset: number,
    _generation: number,
    descriptor: RawFrameDescriptor,
    quantizationConfig: QuantizationConfig,
  ) {
    this.#gpu = gpu;
    this.#descriptor = descriptor;
    this.#quantizationConfig = quantizationConfig;
    this.#presenter = new GpuPreviewPresenter(gpu.context, gpu.device, gpu.canvasFormat);
    this.#rawTexture = gpu.device.createTexture({
      label: 'normal-fused-raw-source',
      size: [descriptor.width, descriptor.height, 1],
      format: 'r16uint',
      usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.#outputTexture = gpu.device.createTexture({
      label: 'normal-fused-output',
      size: [descriptor.width, descriptor.height, 1],
      format: 'rgba32float',
      usage: GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.#uniforms = gpu.device.createBuffer({
      label: 'normal-fused-params',
      size: FUSED_UNIFORM_BYTES,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.uploadFrame(raw, rawByteOffset, descriptor);
  }

  public canReplaceFrame(descriptor: RawFrameDescriptor): boolean {
    return descriptor.width === this.#descriptor.width
      && descriptor.height === this.#descriptor.height
      && descriptor.rowStrideSamples === this.#descriptor.rowStrideSamples;
  }

  public replaceFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    if (!this.canReplaceFrame(descriptor)) {
      throw new Error('GPU_FRAME_EXTENT_CHANGED: executor resources must be rebuilt');
    }
    this.#descriptor = descriptor;
    this.#fullBindGroup = null;
    this.#preBindGroup = null;
    this.#demBindGroup = null;
    this.#postBindGroup = null;
    this.#audit = new TransferAudit();
    this.uploadFrame(raw, rawByteOffset, descriptor);
  }

  public prepare(identity: ExecutionIdentity): void {
    const parameters = packFusedUniforms(this.#descriptor, identity.frameIndex, this.#demosaicParameterValues, this.#quantizationConfig);
    this.#gpu.device.queue.writeBuffer(this.#uniforms, 0, parameters);
    if (this.#demMethod === '00') {
      this.#fullPipeline ??= this.createPipeline(compileFusedNormalShader('00'), 'normal_fused_main');
      this.#fullBindGroup = this.#gpu.device.createBindGroup({
        layout: this.#fullPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.#rawTexture.createView() },
          { binding: 1, resource: this.#outputTexture.createView() },
          { binding: 2, resource: { buffer: this.#uniforms } },
        ],
      });
      return;
    }
    this.prepareSegmented(identity);
  }

  public async execute(_phase: FramePhase, identity: ExecutionIdentity): Promise<PreviewDescriptor> {
    if (this.#demMethod === '00') {
      if (this.#fullPipeline === null || this.#fullBindGroup === null) {
        throw new Error('FUSED_GRAPH_INVALID: fused pipeline was not prepared');
      }
    } else if (this.#prePipeline === null || this.#demPipeline === null || this.#postPipeline === null || this.#preBindGroup === null || this.#demBindGroup === null || this.#postBindGroup === null) {
      throw new Error('FUSED_GRAPH_INVALID: segmented pipeline was not prepared');
    }
    const encoder = this.#gpu.device.createCommandEncoder({ label: 'normal-fused-frame' });
    if (this.#demMethod === '00') {
      this.encodeCompute(encoder, this.#fullPipeline!, this.#fullBindGroup!, 'normal-fused-compute');
    } else {
      this.encodeCompute(encoder, this.#prePipeline!, this.#preBindGroup!, 'normal-fused-pre');
      this.encodeCompute(encoder, this.#demPipeline!, this.#demBindGroup!, 'normal-fused-dem');
      this.encodeCompute(encoder, this.#postPipeline!, this.#postBindGroup!, 'normal-fused-post');
    }
    this.#presenter.encode(encoder, this.#outputTexture);
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#gpu.device.queue.onSubmittedWorkDone();
    const preview = normalManifest.preview_outputs[0];
    if (preview === undefined) throw new Error('PREVIEW_UNAVAILABLE: normal graph has no preview output');
    return {
      nodeId: preview.node_id,
      portId: preview.port_id,
      frameIndex: identity.frameIndex,
      runRevision: identity.runRevision,
      methodRevision: identity.methodRevision,
      gpuGeneration: identity.gpuGeneration,
      width: this.#descriptor.width,
      height: this.#descriptor.height,
      format: 'rgba32float',
      domain: preview.domain,
    };
  }

  public reset(): void {
    this.#presenter.clear();
    this.#fullBindGroup = null;
    this.#preBindGroup = null;
    this.#demBindGroup = null;
    this.#postBindGroup = null;
  }

  public dispose(): void {
    this.#rawTexture.destroy();
    this.#outputTexture.destroy();
    this.#uniforms.destroy();
    this.#preTexture?.destroy();
    this.#demTexture?.destroy();
    this.#demUniforms?.destroy();
  }

  public transferAudit(): TransferAuditSnapshot {
    return this.#audit.snapshot();
  }

  public setMethod(nodeId: string, method: string): void {
    const node = normalManifest.nodes.find((candidate) => candidate.id === nodeId);
    if (node === undefined || !node.methods.some((candidate) => candidate.method === method)) {
      throw new Error(`METHOD_INVALID: ${nodeId}.${method}`);
    }
    if (nodeId === 'dem') {
      if (!DEM_METHODS.includes(method as DemMethod)) throw new Error(`METHOD_INVALID: dem.${method}`);
      this.#demMethod = method as DemMethod;
      this.#fullPipeline = null;
      this.#prePipeline = null;
      this.#demPipeline = null;
      this.#postPipeline = null;
      this.#fullBindGroup = null;
      this.#preBindGroup = null;
      this.#demBindGroup = null;
      this.#postBindGroup = null;
    }
  }

  public setQuantizationConfig(config: QuantizationConfig): void {
    this.#quantizationConfig = config;
    this.#fullBindGroup = null;
    this.#preBindGroup = null;
    this.#demBindGroup = null;
    this.#postBindGroup = null;
  }

  public setParameter(parameter: string, value: number): void {
    if (!(parameter in this.#demosaicParameterValues) || !Number.isFinite(value)) {
      throw new Error(`PARAMETER_INVALID: ${parameter}`);
    }
    this.#demosaicParameterValues[parameter as 'vng_threshold' | 'ahd_l_threshold' | 'ahd_c_threshold_sq'] = value;
    this.#fullBindGroup = null;
    this.#preBindGroup = null;
    this.#demBindGroup = null;
    this.#postBindGroup = null;
  }

  private prepareSegmented(identity: ExecutionIdentity): void {
    if (this.#preTexture === null) {
      this.#preTexture = this.createTexture('normal-fused-pre', 'r32float', GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING);
    }
    if (this.#demTexture === null) {
      this.#demTexture = this.createTexture('normal-fused-dem', 'rgba32float', GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING);
    }
    if (this.#demUniforms === null) {
      this.#demUniforms = this.#gpu.device.createBuffer({ label: 'normal-fused-dem-params', size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    }
    const segmented = compileSegmentedNormalShaders(this.#demMethod);
    this.#prePipeline ??= this.createPipeline(segmented.pre, 'pre_demosaic_main');
    const complexMethod = this.#demMethod as Exclude<DemMethod, '00'>;
    this.#demPipeline ??= this.createPipeline(segmented.dem, DEM_ENTRY_POINTS[complexMethod]);
    this.#postPipeline ??= this.createPipeline(segmented.post, 'postprocess_main');
    const demParams = new ArrayBuffer(32);
    const view = new DataView(demParams);
    const cfa = { rggb: [0, 1, 1, 2], grbg: [1, 0, 2, 1], gbrg: [1, 2, 0, 1], bggr: [2, 1, 1, 0] }[this.#descriptor.cfa];
    cfa.forEach((channel, index) => view.setUint32(index * 4, channel, true));
    view.setFloat32(16, this.#demosaicParameterValues.vng_threshold, true);
    view.setFloat32(20, this.#demosaicParameterValues.ahd_l_threshold, true);
    view.setFloat32(24, this.#demosaicParameterValues.ahd_c_threshold_sq, true);
    this.#gpu.device.queue.writeBuffer(this.#demUniforms, 0, demParams);
    this.#preBindGroup = this.#gpu.device.createBindGroup({ layout: this.#prePipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: this.#rawTexture.createView() }, { binding: 1, resource: this.#preTexture.createView() }, { binding: 2, resource: { buffer: this.#uniforms } }] });
    this.#demBindGroup = this.#gpu.device.createBindGroup({ layout: this.#demPipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: this.#demUniforms } }, { binding: 1, resource: this.#preTexture.createView() }, { binding: 2, resource: this.#demTexture.createView() }] });
    this.#postBindGroup = this.#gpu.device.createBindGroup({ layout: this.#postPipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: this.#demTexture.createView() }, { binding: 1, resource: this.#outputTexture.createView() }, { binding: 2, resource: { buffer: this.#uniforms } }] });
    void identity;
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
    const module = this.#gpu.device.createShaderModule({ code: source });
    return this.#gpu.device.createComputePipeline({ label: entryPoint, layout: 'auto', compute: { module, entryPoint } });
  }

  private encodeCompute(encoder: GPUCommandEncoder, pipeline: GPUComputePipeline, bindGroup: GPUBindGroup, label: string): void {
    const pass = encoder.beginComputePass({ label });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8));
    pass.end();
  }
}
