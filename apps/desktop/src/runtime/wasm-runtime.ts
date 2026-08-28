import initWasm, { NormalRuntime } from '../../../../crates/rime-wasm/pkg/rime_wasm.js';

import type { RuntimeEnvelope } from '../../../../web/src/contracts.js';

interface RustRuntimeSnapshot {
  readonly lifecycle_state: RuntimeEnvelope['lifecycleState'];
  readonly run_revision: number;
  readonly method_revision: number;
  readonly gpu_generation: number;
  readonly frame_index: number | null;
  readonly frame_phase: RuntimeEnvelope['framePhase'];
  readonly visible_frame: number | null;
}

export class WasmRuntimeAuthority {
  readonly #runtime: NormalRuntime;
  readonly #graphInstanceId: number;

  private constructor(runtime: NormalRuntime, graphInstanceId: number) {
    this.#runtime = runtime;
    this.#graphInstanceId = graphInstanceId;
  }

  public static async create(graphInstanceId = 1): Promise<WasmRuntimeAuthority> {
    await initWasm();
    return new WasmRuntimeAuthority(new NormalRuntime(), graphInstanceId);
  }

  public load(): RuntimeEnvelope {
    return this.map(this.#runtime.load());
  }

  public run(): RuntimeEnvelope {
    return this.map(this.#runtime.run());
  }

  public step(): RuntimeEnvelope {
    return this.map(this.#runtime.step());
  }

  public completeWarmup(): RuntimeEnvelope {
    return this.map(this.#runtime.complete_warmup());
  }

  public completeOutput(): RuntimeEnvelope {
    return this.map(this.#runtime.complete_output());
  }

  public reset(): RuntimeEnvelope {
    return this.map(this.#runtime.reset());
  }
  public changeMethod(): RuntimeEnvelope {
    return this.map(this.#runtime.change_method());
  }

  public fail(): RuntimeEnvelope {
    return this.map(this.#runtime.fail());
  }

  public deviceLost(): RuntimeEnvelope {
    return this.map(this.#runtime.device_lost());
  }


  private map(serialized: string): RuntimeEnvelope {
    const snapshot = JSON.parse(serialized) as RustRuntimeSnapshot;
    return {
      graphInstanceId: this.#graphInstanceId,
      runRevision: snapshot.run_revision,
      methodRevision: snapshot.method_revision,
      frameIndex: snapshot.frame_index,
      framePhase: snapshot.frame_phase,
      visibleFrameCommitted: snapshot.visible_frame !== null,
      lifecycleState: snapshot.lifecycle_state,
      gpuGeneration: snapshot.gpu_generation,
    };
  }
}
