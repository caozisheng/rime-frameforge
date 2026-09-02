import { describe, expect, it } from 'vitest';

import { RuntimeController } from '../src/runtime-controller.js';
import type { FramePhase, PreviewDescriptor } from '../src/contracts.js';

class RecordingExecutor {
  readonly phases: FramePhase[] = [];
  prepareCalls = 0;

  async prepare(_identity: { frameIndex: number; runRevision: number; methodRevision: number; gpuGeneration: number }): Promise<void> {
    this.prepareCalls += 1;
  }

  async execute(phase: FramePhase, identity: { frameIndex: number; runRevision: number; methodRevision: number; gpuGeneration: number }): Promise<readonly PreviewDescriptor[]> {
    this.phases.push(phase);
    return [{
      nodeId: 'rgb2yuv',
      portId: 'out',
      frameIndex: identity.frameIndex,
      runRevision: identity.runRevision,
      methodRevision: identity.methodRevision,
      gpuGeneration: identity.gpuGeneration,
      width: 32,
      height: 24,
      format: 'rgba32_float',
      domain: 'yuv',
      range: 'normalized',
      channelLayout: 'rgba',
      presentation: 'yuv',
    }];
  }

  reset(): void {}
}

describe('RuntimeController', () => {
  it('prepares once and executes one output frame', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (committed) => previews.push(...committed));

    await controller.step({ frameIndex: 2, runRevision: 1, methodRevision: 1, gpuGeneration: 1 });

    expect(executor.prepareCalls).toBe(1);
    expect(executor.phases).toEqual(['output']);
    expect(previews[0]?.frameIndex).toBe(2);
  });

  it('publishes only the output preview', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (committed) => previews.push(...committed));

    await controller.step({ frameIndex: 0, runRevision: 1, methodRevision: 1, gpuGeneration: 1 });

    expect(previews).toHaveLength(1);
  });

  it('publishes the authoritative execution identity', async () => {
    const executor = new RecordingExecutor();
    const previews: PreviewDescriptor[] = [];
    const controller = new RuntimeController(executor, (committed) => previews.push(...committed));

    await controller.step({ frameIndex: 7, runRevision: 9, methodRevision: 4, gpuGeneration: 3 });

    expect(previews[0]).toMatchObject({ frameIndex: 7, runRevision: 9, methodRevision: 4, gpuGeneration: 3 });
  });

  it('reports preparation and output as separate lifecycle phases', async () => {
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
