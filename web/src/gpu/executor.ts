import { normalManifest } from '../generated/normal_manifest.generated.js';
import quantizeShader from '../../../crates/rime-quant/shaders/quantize.wgsl?raw';
import colorCorrectionShader from '../../../crates/rime-isp/src/vbe/color_correction/color_correction_00.wgsl?raw';
import demBilinearShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_00.wgsl?raw';
import demMhcShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_01.wgsl?raw';
import demPpgShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_02.wgsl?raw';
import demVngShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_03.wgsl?raw';
import demAhdShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_04.wgsl?raw';
import gammaShader from '../../../crates/rime-isp/src/vbe/gamma/gamma_00.wgsl?raw';
import identityR32Shader from '../../../crates/rime-isp/src/shaders/identity_r32.wgsl?raw';
import identityRgba32Shader from '../../../crates/rime-isp/src/shaders/identity_rgba32.wgsl?raw';
import rgb2yuvShader from '../../../crates/rime-isp/src/vbe/rgb_to_yuv/rgb_to_yuv_00.wgsl?raw';
import blcShader from '../../../crates/rime-isp/src/vfe/blc/blc_00.wgsl?raw';
import wbcShader from '../../../crates/rime-isp/src/vbe/white_balance/white_balance_00.wgsl?raw';
import type { FramePhase, PreviewDescriptor, RawFrameDescriptor, TransferAuditSnapshot } from '../contracts.js';
import type { ExecutionIdentity } from '../runtime-controller.js';
import { buildGpuQuantizationPlans, defaultQuantizationConfig, type GpuQuantizationPlan, type QuantizationConfig } from './quantization.js';
import { GpuPreviewPresenter } from './presenter.js';
import { GpuResourceRegistry, type GpuResourceRef } from './resource-registry.js';
import { GpuTexturePool, type TextureLease } from './texture-pool.js';
import { TransferAudit } from './transfer-audit.js';
import type { GpuContext } from './device.js';
import { resolveShaderEntry } from './method-selection.js';
const QUANTIZE_COMMON = quantizeShader.slice(0, quantizeShader.indexOf('@group(0)'));
const QUANTIZE_RGBA_SHADER = `${QUANTIZE_COMMON}
@group(0) @binding(0) var<uniform> quant_params: QuantParams;
@group(0) @binding(1) var quant_input: texture_2d<f32>;
@group(0) @binding(2) var quant_output: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8)
fn quantize_rgba32_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let dimensions = textureDimensions(quant_output);
  if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
  let ppc = max(quant_params.ppc, 1u);
  let pixel_group = id.y * quant_params.groups_per_row + id.x / ppc;
  let ppc_lane = id.x % ppc;
  let sample = textureLoad(quant_input, vec2<i32>(id.xy), 0);
  var result: vec4<f32>;
  var params = quant_params;
  params.channel = 0u;
  result.r = quantize_sample(sample.r, params, pixel_group, ppc_lane);
  params.channel = 1u;
  result.g = quantize_sample(sample.g, params, pixel_group, ppc_lane);
  params.channel = 2u;
  result.b = quantize_sample(sample.b, params, pixel_group, ppc_lane);
  params.channel = 3u;
  result.a = quantize_sample(sample.a, params, pixel_group, ppc_lane);
  textureStore(quant_output, vec2<i32>(id.xy), result);
}`;
const SHADER_BY_ENTRY: Record<string, string> = {
  quantize_r32_main: quantizeShader,
  quantize_rgba32_main: QUANTIZE_RGBA_SHADER,
  blc_main: blcShader,
  identity_r32_main: identityR32Shader,
  identity_rgba32_main: identityRgba32Shader,
  wbc_main: wbcShader,
  demosaic_bilinear_main: demBilinearShader,
  demosaic_mhc_main: demMhcShader,
  demosaic_ppg_main: demPpgShader,
  demosaic_vng_main: demVngShader,
  demosaic_ahd_main: demAhdShader,
  color_correction_main: colorCorrectionShader,
  gamma_main: gammaShader,
  rgb2yuv_main: rgb2yuvShader,
};

const FORMAT_BY_NAME: Record<string, GPUTextureFormat> = {
  r16_uint: 'r16uint',
  r32_float: 'r32float',
  rgba32_float: 'rgba32float',
};

export class NormalGpuExecutor {
  readonly #gpu: GpuContext;
  #audit = new TransferAudit();
  readonly #registry: GpuResourceRegistry;
  readonly #presenter: GpuPreviewPresenter;
  readonly #pipelines = new Map<string, GPUComputePipeline>();
  readonly #pool: GpuTexturePool;
  #descriptor: RawFrameDescriptor;
  readonly #blcParams: GPUBuffer;
  readonly #demosaicParams: GPUBuffer;
  readonly #rawTexture: GPUTexture;
  #quantizationConfig: QuantizationConfig = defaultQuantizationConfig;
  #rawRef: GpuResourceRef;
  #generation: number;
  readonly #selectedMethods: Record<string, string> = {};
  readonly #demosaicParameterValues = {
    vng_threshold: 1.5,
    ahd_l_threshold: 2.0,
    ahd_c_threshold_sq: 4.0,
  };
  public constructor(gpu: GpuContext, raw: ArrayBuffer, rawByteOffset: number, generation: number, descriptor: RawFrameDescriptor) {
    this.#gpu = gpu;
    this.#generation = generation;
    this.#descriptor = descriptor;
    this.#registry = new GpuResourceRegistry(this.#generation);
    this.#presenter = new GpuPreviewPresenter(gpu.context, gpu.device, gpu.canvasFormat);
    this.#pool = new GpuTexturePool(this.#generation);
    this.#rawTexture = gpu.device.createTexture({
      label: 'normal-raw-source',
      size: [descriptor.width, descriptor.height, 1],
      format: 'r16uint',
      usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.#rawRef = this.#registry.register('raw_source.out', this.#rawTexture, { destroyOnInvalidate: false });
    this.#blcParams = gpu.device.createBuffer({
      label: 'blc-params',
      size: 16,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.#demosaicParams = gpu.device.createBuffer({
      label: 'demosaic-params',
      size: 32,
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
    this.#generation += 1;
    this.#registry.invalidate(this.#generation);
    this.#pool.invalidateGeneration(this.#generation);
    this.#rawRef = this.#registry.register('raw_source.out', this.#rawTexture, { destroyOnInvalidate: false });
    this.#audit = new TransferAudit();
    this.uploadFrame(raw, rawByteOffset, descriptor);
  }

  public dispose(): void {
    this.#registry.invalidate(this.#generation + 1);
    this.#pool.dispose();
    this.#rawTexture.destroy();
    this.#blcParams.destroy();
    this.#demosaicParams.destroy();
  }

  private uploadFrame(raw: ArrayBuffer, rawByteOffset: number, descriptor: RawFrameDescriptor): void {
    const expected = descriptor.rowStrideSamples * descriptor.height;
    if (rawByteOffset % 2 !== 0 || rawByteOffset < 0 || rawByteOffset + expected * 2 !== raw.byteLength) {
      throw new Error(`INPUT_INVALID: expected ${expected} RAW samples`);
    }
    const rawSamples = new Uint16Array(raw, rawByteOffset, expected);
    const bytes = new ArrayBuffer(16);
    const view = new DataView(bytes);
    view.setFloat32(0, descriptor.blackLevel, true);
    view.setFloat32(4, descriptor.whiteLevel, true);
    view.setUint32(8, descriptor.width, true);
    view.setUint32(12, descriptor.height, true);
    this.#gpu.device.queue.writeBuffer(this.#blcParams, 0, bytes);
    this.writeDemosaicParams(descriptor);
    this.#gpu.device.queue.writeTexture(
      { texture: this.#rawTexture },
      rawSamples,
      { bytesPerRow: descriptor.width * 2, rowsPerImage: descriptor.height },
      { width: descriptor.width, height: descriptor.height, depthOrArrayLayers: 1 },
    );
    this.#audit.recordRawUpload(rawSamples.byteLength);
  }

  public async execute(phase: FramePhase, identity: ExecutionIdentity): Promise<PreviewDescriptor> {
    let previousTexture = this.#rawTexture;
    let previousLease: TextureLease | null = null;
    const plans = buildGpuQuantizationPlans(
      this.#quantizationConfig,
      this.#descriptor.width,
      this.#descriptor.height,
      identity.frameIndex,
    );

    for (const node of normalManifest.nodes.slice(1)) {
      const output = node.outputs[0];
      const shaderEntry = resolveShaderEntry(node, this.#selectedMethods);
      if (output === undefined || shaderEntry === null) throw new Error(`MANIFEST_INVALID: node ${node.id}`);
      const pipeline = this.pipeline(shaderEntry);
      const format = FORMAT_BY_NAME[output.format];
      if (format === undefined) throw new Error(`MANIFEST_INVALID: unsupported format ${output.format}`);
      const usage = GPUTextureUsage.STORAGE_BINDING | GPUTextureUsage.TEXTURE_BINDING;
      const outputLease = this.#pool.acquire(
        { domain: output.domain, format, width: this.#descriptor.width, height: this.#descriptor.height, usage },
        () => this.#gpu.device.createTexture({
          label: `${node.id}.pooled`,
          size: [this.#descriptor.width, this.#descriptor.height, 1],
          format,
          usage,
        }),
      );
      const outputTexture = outputLease.texture;
      const bindGroup = this.#gpu.device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: node.id === 'blc'
          ? [
              { binding: 0, resource: { buffer: this.#blcParams } },
              { binding: 1, resource: previousTexture.createView() },
              { binding: 2, resource: outputTexture.createView() },
            ]
          : node.id === 'dem'
            ? [
                { binding: 0, resource: { buffer: this.#demosaicParams } },
                { binding: 1, resource: previousTexture.createView() },
                { binding: 2, resource: outputTexture.createView() },
              ]
            : [
                { binding: 0, resource: previousTexture.createView() },
                { binding: 1, resource: outputTexture.createView() },
              ],
      });
      const encoder = this.#gpu.device.createCommandEncoder({ label: `${node.id}.${phase}` });
      const pass = encoder.beginComputePass({ label: `${node.id}.${phase}` });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.dispatchWorkgroups(Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8));
      pass.end();
      this.#gpu.device.queue.submit([encoder.finish()]);
      await this.#gpu.device.queue.onSubmittedWorkDone();

      const plan = plans.get(node.id);
      if (plan?.outputEnabled === true) {
        const quantizedLease = this.#pool.acquire(
          { domain: output.domain, format, width: this.#descriptor.width, height: this.#descriptor.height, usage },
          () => this.#gpu.device.createTexture({
            label: `${node.id}.rime-q`,
            size: [this.#descriptor.width, this.#descriptor.height, 1],
            format,
            usage,
          }),
        );
        await this.quantizeOutput(outputTexture, quantizedLease, format, plan);
        this.#pool.release(outputLease);
        if (previousLease !== null) this.#pool.release(previousLease);
        previousTexture = quantizedLease.texture;
        previousLease = quantizedLease;
      } else {
        if (previousLease !== null) this.#pool.release(previousLease);
        previousTexture = outputTexture;
        previousLease = outputLease;
      }
    }

    if (previousLease === null) throw new Error('NODE_EXECUTION_FAILED: no final texture');
    if (phase === 'output') await this.#presenter.render(previousTexture);
    this.#pool.release(previousLease);
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
    this.#generation += 1;
    this.#presenter.clear();
    this.#registry.invalidate(this.#generation);
    this.#pool.invalidateGeneration(this.#generation);
    this.#rawRef = this.#registry.register('raw_source.out', this.#rawTexture, { destroyOnInvalidate: false });
  }

  public transferAudit(): TransferAuditSnapshot { return this.#audit.snapshot(); }
  public setMethod(nodeId: string, method: string): void {
    const node = normalManifest.nodes.find((candidate) => candidate.id === nodeId);
    if (node === undefined || !node.methods.some((candidate) => candidate.method === method)) {
      throw new Error(`METHOD_INVALID: ${nodeId}.${method}`);
    }
    this.#selectedMethods[nodeId] = method;
  }

  public setQuantizationConfig(config: QuantizationConfig): void {
    this.#quantizationConfig = config;
  }

  private async quantizeOutput(
    input: GPUTexture,
    outputLease: TextureLease,
    format: GPUTextureFormat,
    plan: GpuQuantizationPlan,
  ): Promise<void> {
    const entryPoint = format === 'r32float' ? 'quantize_r32_main' : 'quantize_rgba32_main';
    const pipeline = this.pipeline(entryPoint);
    const params = this.#gpu.device.createBuffer({
      label: `${plan.moduleId}.rime-q-params`,
      size: 64,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const bytes = new ArrayBuffer(64);
    const view = new DataView(bytes);
    view.setFloat32(0, plan.scale, true);
    view.setFloat32(4, plan.qmin, true);
    view.setFloat32(8, plan.qmax, true);
    view.setUint32(12, plan.roundingMode, true);
    view.setUint32(16, plan.seed, true);
    view.setUint32(20, plan.streamId, true);
    view.setUint32(24, plan.frameIndex, true);
    view.setUint32(28, plan.plane, true);
    view.setUint32(32, this.#descriptor.width, true);
    view.setUint32(36, this.#descriptor.height, true);
    view.setUint32(40, plan.ppc, true);
    view.setUint32(44, 0, true);
    view.setUint32(48, plan.groupsPerRow, true);
    view.setUint32(52, plan.groupsPerFrame, true);
    this.#gpu.device.queue.writeBuffer(params, 0, bytes);
    const bindGroup = this.#gpu.device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: params } },
        { binding: 1, resource: input.createView() },
        { binding: 2, resource: outputLease.texture.createView() },
      ],
    });
    const encoder = this.#gpu.device.createCommandEncoder({ label: `${plan.moduleId}.rime-q` });
    const pass = encoder.beginComputePass({ label: `${plan.moduleId}.rime-q` });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(Math.ceil(this.#descriptor.width / 8), Math.ceil(this.#descriptor.height / 8));
    pass.end();
    this.#gpu.device.queue.submit([encoder.finish()]);
    await this.#gpu.device.queue.onSubmittedWorkDone();
    params.destroy();
  }

  public setParameter(parameter: string, value: number): void {
    if (!(parameter in this.#demosaicParameterValues) || !Number.isFinite(value)) {
      throw new Error(`PARAMETER_INVALID: ${parameter}`);
    }
    this.#demosaicParameterValues[parameter as 'vng_threshold' | 'ahd_l_threshold' | 'ahd_c_threshold_sq'] = value;
    this.writeDemosaicParams(this.#descriptor);
  }

  private writeDemosaicParams(descriptor: RawFrameDescriptor): void {
    const cfaPattern: Record<RawFrameDescriptor['cfa'], readonly number[]> = {
      rggb: [0, 1, 1, 2],
      grbg: [1, 0, 2, 1],
      gbrg: [1, 2, 0, 1],
      bggr: [2, 1, 1, 0],
    };
    const bytes = new ArrayBuffer(32);
    const view = new DataView(bytes);
    cfaPattern[descriptor.cfa].forEach((channel, index) => view.setUint32(index * 4, channel, true));
    view.setFloat32(16, this.#demosaicParameterValues.vng_threshold, true);
    view.setFloat32(20, this.#demosaicParameterValues.ahd_l_threshold, true);
    view.setFloat32(24, this.#demosaicParameterValues.ahd_c_threshold_sq, true);
    this.#gpu.device.queue.writeBuffer(this.#demosaicParams, 0, bytes);
  }

  private pipeline(entryPoint: string): GPUComputePipeline {
    const cached = this.#pipelines.get(entryPoint);
    if (cached !== undefined) return cached;
    const shader = SHADER_BY_ENTRY[entryPoint];
    if (shader === undefined) throw new Error(`SHADER_ENTRY_MISSING: ${entryPoint}`);
    const pipeline = this.#gpu.device.createComputePipeline({
      layout: 'auto',
      compute: { module: this.#gpu.device.createShaderModule({ code: shader }), entryPoint },
    });
    this.#pipelines.set(entryPoint, pipeline);
    return pipeline;
  }
}
