import { describe, expect, it } from 'vitest';

import { acceptsEnvelope } from '../src/revision-guard.js';
import type { RuntimeEnvelope } from '../src/contracts.js';

const current: RuntimeEnvelope = {
  graphInstanceId: 4,
  runRevision: 7,
  methodRevision: 1,
  frameIndex: 0,
  framePhase: 'output',
  visibleFrameCommitted: true,
  lifecycleState: 'completed',
  gpuGeneration: 2,
};

describe('acceptsEnvelope', () => {
  it('rejects an event from an older run', () => {
    expect(acceptsEnvelope(current, { ...current, runRevision: 6 })).toBe(false);
  });

  it('rejects an event from an older GPU generation', () => {
    expect(acceptsEnvelope(current, { ...current, gpuGeneration: 1 })).toBe(false);
  });

  it('accepts a newer run from the same graph', () => {
    expect(acceptsEnvelope(current, { ...current, runRevision: 8 })).toBe(true);
  });
});
