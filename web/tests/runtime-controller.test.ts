import { describe, expect, it } from 'vitest';

import { RuntimeController } from '../src/runtime-controller.js';
import type { FramePhase, PreviewDescriptor } from '../src/contracts.js';

class RecordingExecutor {
  readonly phases: FramePhase[] = [];

  async execute(phase: FramePhase, identity: { frameIndex: number; runRevision: number; methodRevision: number; gpuGeneration: number }): Promise<PreviewDescriptor> {
    this.phases.push(phase);
    return {
      nodeId: 'rgb2yuv',
      portId: 'out',
      frameIndex: identity.frameIndex,
      runRevision: identity.runRevision,
      methodRevision: identity.methodRevision,
      gpuGeneration: identity.gpuGeneration,
      width: 32,
      height: 24,
      format: 'rgba32float',
      domain: 'yuv',
    };
  }

  reset(): void {}
}

describe('RuntimeController', () => {
  it('executes warmup before output', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (preview) => previews.push(preview));

    await controller.step({ frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 });

    expect(executor.phases).toEqual(['warmup', 'output']);
  });

  it('publishes only the output preview', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (preview) => previews.push(preview));

    await controller.step({ frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 });

    expect(previews).toHaveLength(1);
  });

  it('publishes the authoritative execution identity', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (preview) => previews.push(preview));

    await controller.step({ frameIndex: 7, runRevision: 9, methodRevision: 4, gpuGeneration: 3 });

    expect(previews[0]).toMatchObject({ frameIndex: 7, runRevision: 9, methodRevision: 4, gpuGeneration: 3 });
  });

  it('reports warmup and output as separate phases', async () => {
    const executor = new RecordingExecutor();
    const phases: FramePhase[] = [];
    const controller = new RuntimeController(
      executor,
      () => undefined,
      (phase) => phases.push(phase),
    );

    await controller.step({ frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 });

    expect(phases).toEqual(['warmup', 'output']);
  });
});
