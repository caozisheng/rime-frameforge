import type { FramePhase, PreviewDescriptor } from './contracts.js';

export interface ExecutionIdentity {
  readonly frameIndex: number;
  readonly runRevision: number;
  readonly methodRevision: number;
  readonly gpuGeneration: number;
}
export interface FrameExecutor {
  prepare(identity: ExecutionIdentity): Promise<void> | void;
  execute(phase: FramePhase, identity: ExecutionIdentity): Promise<readonly PreviewDescriptor[]>;
  reset(): void;
}

export class RuntimeController {
  readonly #executor: FrameExecutor;
  readonly #publishPreview: (previews: readonly PreviewDescriptor[]) => void;
  readonly #publishPhase: (phase: FramePhase) => void;
  readonly #completeFrame: () => Promise<void> | void;
  readonly #abortFrame: () => Promise<void> | void;
  public constructor(
    executor: FrameExecutor,
    publishPreview: (previews: readonly PreviewDescriptor[]) => void,
    publishPhase: (phase: FramePhase) => void = () => undefined,
    completeFrame: () => Promise<void> | void = () => undefined,
    abortFrame: () => Promise<void> | void = () => undefined,
  ) {
    this.#executor = executor;
    this.#publishPreview = publishPreview;
    this.#publishPhase = publishPhase;
    this.#completeFrame = completeFrame;
    this.#abortFrame = abortFrame;
  }
  public async step(identity: ExecutionIdentity): Promise<void> {
    try {
      await this.#executor.prepare(identity);
      this.#publishPhase('warmup');
      const outputs = await this.#executor.execute('output', identity);
      await this.#completeFrame();
      this.#publishPhase('output');
      this.#publishPreview(outputs);
    } catch (error) {
      await this.#abortFrame();
      throw error;
    }
  }

  public reset(): void {
    this.#executor.reset();
  }
}
