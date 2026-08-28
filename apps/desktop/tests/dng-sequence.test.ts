import { describe, expect, it } from 'vitest';

import { canLoadNextDngFrame, commitPendingDngFrame, nextDngSequenceFrame, shouldCommitDngDescriptor, shouldResumeDngSequence, visibleDngFrameIndex } from '../src/runtime/dng-sequence.js';

describe('DNG sequence playback', () => {
  it('advances to the next frame only while playback is active', () => {
    expect(nextDngSequenceFrame(0, 3, true)).toBe(1);
    expect(nextDngSequenceFrame(1, 3, false)).toBeNull();
    expect(nextDngSequenceFrame(2, 3, true)).toBeNull();
  });

  it('allows loading the next frame after the visible frame completes', () => {
    expect(canLoadNextDngFrame('completed')).toBe(true);
    expect(canLoadNextDngFrame('stop')).toBe(true);
    expect(canLoadNextDngFrame('running')).toBe(false);
  });

  it('does not resume after playback was stopped during async loading', () => {
    expect(shouldResumeDngSequence(true, 2, 2)).toBe(true);
    expect(shouldResumeDngSequence(false, 2, 2)).toBe(false);
    expect(shouldResumeDngSequence(true, 2, 1)).toBe(false);
  });

  it('keeps the visible descriptor frozen when pause wins an async load race', () => {
    expect(shouldCommitDngDescriptor(true, 2, 2)).toBe(true);
    expect(shouldCommitDngDescriptor(false, 2, 2)).toBe(false);
    expect(shouldCommitDngDescriptor(true, 2, 1)).toBe(false);
  });

  it('keeps the visible frame index stable until the next descriptor commits', () => {
    expect(visibleDngFrameIndex(0, 1, false)).toBe(0);
    expect(visibleDngFrameIndex(0, 1, true)).toBe(1);
  });

  it('commits a pending frame index together with its descriptor', () => {
    const descriptor = { frameIndex: 1, fileName: 'frame-001.dng' };
    expect(commitPendingDngFrame({ index: 1, descriptor }, 0)).toEqual({ index: 1, descriptor });
    expect(commitPendingDngFrame({ index: 1, descriptor }, 1)).toBeNull();
  });
});
