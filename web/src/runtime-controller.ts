import type { FramePhase, PreviewDescriptor } from './contracts.js';

export interface ExecutionIdentity {
  readonly frameIndex: number;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly gpuGeneration: number;
}

export interface FrameExecutor {
  execute(phase: FramePhase, identity: ExecutionIdentity): Promise<PreviewDescriptor>;
  reset(): void;
}

export class RuntimeController {
  readonly #executor: FrameExecutor;
  readonly #publishPreview: (preview: PreviewDescriptor) => void;
  readonly #publishPhase: (phase: FramePhase) => void;

  public constructor(
    executor: FrameExecutor,
    publishPreview: (preview: PreviewDescriptor) => void,
    publishPhase: (phase: FramePhase) => void = () => undefined,
  ) {
    this.#executor = executor;
    this.#publishPreview = publishPreview;
    this.#publishPhase = publishPhase;
  }

  public async step(identity: ExecutionIdentity): Promise<void> {
    await this.#executor.execute('warmup', identity);
    this.#publishPhase('warmup');
    const output = await this.#executor.execute('output', identity);
    this.#publishPhase('output');
    this.#publishPreview(output);
  }

  public reset(): void {
    this.#executor.reset();
  }
}
