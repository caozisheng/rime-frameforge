import initWasm, { FramePacketDeriver, NormalRuntime } from '../../../../crates/rime-wasm/pkg/rime_wasm.js';

import type { DrcIqParameters, FrameBeginPackets, FrameConsumerPackets, FrameIdentity, FrameMode, RawFrameDescriptor, RuntimeEnvelope } from '../../../../web/src/contracts.js';

interface RustRuntimeSnapshot {
  readonly lifecycle_state: RuntimeEnvelope['lifecycleState'];
  readonly run_revision: number;
  readonly method_revision: number;
  readonly config_revision: number;
  readonly gpu_generation: number;
  readonly frame_index: number | null;
  readonly frame_phase: RuntimeEnvelope['framePhase'];
  readonly visible_frame: number | null;
}

export class WasmRuntimeAuthority {
  readonly #runtime: NormalRuntime;
  readonly #packetDeriver: FramePacketDeriver;
  readonly #graphInstanceId: number;
  #framePending = false;

  private constructor(runtime: NormalRuntime, packetDeriver: FramePacketDeriver, graphInstanceId: number) {
    this.#runtime = runtime;
    this.#packetDeriver = packetDeriver;
    this.#graphInstanceId = graphInstanceId;
  }

  public static async create(graphInstanceId = 1): Promise<WasmRuntimeAuthority> {
    await initWasm();
    return new WasmRuntimeAuthority(new NormalRuntime(), new FramePacketDeriver(), graphInstanceId);
  }

  public load(): RuntimeEnvelope {
    return this.map(this.#runtime.load());
  }

  public run(frameIndex = 0): RuntimeEnvelope {
    return this.map(this.#runtime.run_frame(frameIndex));
  }

  public step(frameIndex = 0): RuntimeEnvelope {
    return this.map(this.#runtime.step_frame(frameIndex));
  }
  public runFrame(frameIndex: number): RuntimeEnvelope {
    return this.map(this.#runtime.run_frame(frameIndex));
  }

  public completeWarmup(): RuntimeEnvelope {
    return this.map(this.#runtime.complete_warmup());
  }

  public completeOutput(): RuntimeEnvelope {
    return this.map(this.#runtime.complete_output());
  }

  public reset(): RuntimeEnvelope {
    this.#packetDeriver.reset();
    this.#framePending = false;
    return this.map(this.#runtime.reset());
  }
  public changeMethod(): RuntimeEnvelope {
    return this.map(this.#runtime.change_method());
  }
  public changeConfig(): RuntimeEnvelope {
    const runtime = this.#runtime as NormalRuntime & { change_config(): string };
    return this.map(runtime.change_config());
  }
  public setQuantizationConfig(config: string): RuntimeEnvelope {
    return this.map(this.#runtime.set_quantization_config(config));
  }
  public quantizationConfig(): string {
    return this.#runtime.quantization_config_json();
  }

  public beginFrame(
    descriptor: RawFrameDescriptor,
    raw: ArrayBuffer,
    rawByteOffset: number,
    identity: FrameIdentity,
    mode: FrameMode,
    methods: Readonly<Record<string, string>>,
    parameters: Readonly<Record<string, number>>,
    gammaLut: readonly number[],
    drc: DrcIqParameters,
    bypassModules: readonly { readonly module_id: string; readonly bypass: boolean }[],
  ): FrameBeginPackets {
    if (this.#framePending) {
      throw new Error('WASM_FRAME_IN_PROGRESS: complete or abort the pending frame before beginning another');
    }
    const sampleCount = descriptor.rowStrideSamples * descriptor.height;
    const rawSamples = new Uint16Array(raw, rawByteOffset, sampleCount);
    const options = {
      dem_method: methods.dem ?? '00',
      drc_method: methods.drc ?? '00',
      drc_gain_offset_ev: drc.drc_gain_offset_ev,
      drc_knee: drc.knee,
      drc_amplifier: drc.amplifier,
      drc_modulation_curves: drc.edge_curve !== undefined && drc.luma_curve !== undefined
        ? { edge: drc.edge_curve, luma: drc.luma_curve }
        : null,
      drc_details_amplify: parameters.enable_details_amplify !== 0,
      wbc_highlight_recovery: parameters.enable_highlight_recovery !== 0,
      gamma: { gamma: parameters.gamma ?? 2.2, lut: gammaLut },
      dem_thresholds: {
        vng_threshold: parameters.vng_threshold ?? 1.5,
        ahd_l_threshold: parameters.ahd_l_threshold ?? 2,
        ahd_c_threshold_sq: parameters.ahd_c_threshold_sq ?? 4,
      },
      quantization: JSON.parse(this.quantizationConfig()),
      bypass_modules: Object.fromEntries(bypassModules.map((module) => [module.module_id, module.bypass])),
    };
    const begin = this.#packetDeriver.begin_frame(
      JSON.stringify(descriptor),
      rawSamples,
      JSON.stringify(options),
      toWasmU64(identity.frameIndex, 'frameIndex'),
      toWasmU64(identity.runRevision, 'runRevision'),
      toWasmU64(identity.methodRevision, 'methodRevision'),
      mode,
    );
    this.#framePending = true;
    try {
      return {
        blcUniform: copyPacketBytes(begin.blc_uniform()),
        lcstUniform: copyPacketBytes(begin.lcst_uniform()),
      };
    } finally {
      begin.free();
    }
  }

  public prepareConsumers(payload: Uint8Array | undefined, sequenceColdStart: boolean): FrameConsumerPackets {
    this.requirePendingFrame();
    const packets = this.#packetDeriver.prepare_consumers(payload, sequenceColdStart);
    try {
      return {
        tintlessUniform: copyPacketBytes(packets.tintless_uniform()),
        tintlessMesh: copyPacketBytes(packets.tintless_mesh()),
        tintlessAudit: copyPacketBytes(packets.tintless_audit()),
        lscUniform: copyPacketBytes(packets.lsc_uniform()),
        lscMeshHeaders: copyPacketBytes(packets.lsc_mesh_headers()),
        lscMeshEntries: copyPacketBytes(packets.lsc_mesh_entries()),
        lscActive: packets.lsc_active(),
        wbcUniform: copyPacketBytes(packets.wbc_uniform()),
        drcUniform: copyPacketBytes(packets.drc_uniform()),
        demUniform: copyPacketBytes(packets.dem_uniform()),
        drcGlobalLut: copyPacketBytes(packets.drc_global_lut()),
        drcLocalLut: copyPacketBytes(packets.drc_local_lut()),
        drcModulationLuts: copyPacketBytes(packets.drc_modulation_luts()),
        fusedUniform: copyPacketBytes(packets.fused_uniform()),
        colorReproduceHsLut: copyPacketBytes(packets.color_reproduce_hs_lut()),
        preprocessSnapshotJson: packets.preprocess_snapshot_json(),
      };
    } finally {
      packets.free();
    }
  }

  public stageLcstStatistics(payload: Uint8Array): void {
    this.requirePendingFrame();
    this.#packetDeriver.stage_lcst_statistics(payload);
  }

  private requirePendingFrame(): void {
    if (!this.#framePending) throw new Error('WASM_FRAME_NOT_PREPARED: begin a frame first');
  }

  public abortFrame(): void {
    if (!this.#framePending) return;
    try {
      this.#packetDeriver.abort_frame();
    } finally {
      this.#framePending = false;
    }
  }

  public completeFrame(): void {
    if (!this.#framePending) return;
    this.#packetDeriver.complete_frame();
    this.#framePending = false;
  }

  public dispose(): void {
    try {
      this.#packetDeriver.reset();
    } finally {
      this.#framePending = false;
      this.#packetDeriver.free();
      this.#runtime.free();
    }
  }
  public deviceLost(): RuntimeEnvelope {
    try {
      this.#packetDeriver.reset();
    } finally {
      this.#framePending = false;
    }
    return this.map(this.#runtime.device_lost());
  }

  public fail(): RuntimeEnvelope {
    return this.map(this.#runtime.fail());
  }
  private map(serialized: string): RuntimeEnvelope {
    const snapshot = JSON.parse(serialized) as RustRuntimeSnapshot;
    return {
      graphInstanceId: this.#graphInstanceId,
      runRevision: snapshot.run_revision,
      configRevision: snapshot.config_revision,
      methodRevision: snapshot.method_revision,
      frameIndex: snapshot.frame_index,
      framePhase: snapshot.frame_phase,
      visibleFrameCommitted: snapshot.visible_frame !== null,
      lifecycleState: snapshot.lifecycle_state,
      gpuGeneration: snapshot.gpu_generation,
    };
  }
}

function copyPacketBytes(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  return new Uint8Array(bytes);
}

function toWasmU64(value: number | bigint, field: string): bigint {
  if (typeof value === 'bigint') {
    if (value >= 0n && value <= 0xffff_ffff_ffff_ffffn) return value;
  } else if (Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  throw new Error(`WASM_FRAME_IDENTITY_INVALID: ${field} must be an unsigned 64-bit integer`);
}
