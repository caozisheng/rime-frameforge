import { describe, expect, it } from 'vitest';

import { canLoadNextDngFrame, commitPendingDngFrame, dngFrameForStep, isCurrentDngPrefetch, nextDngRunFrame, nextDngSequenceFrame, shouldCommitDngDescriptor, shouldResumeDngSequence, visibleDngFrameIndex } from '../src/runtime/dng-sequence.js';

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

  it('uses the committed frame as the next playback source', () => {
    expect(nextDngSequenceFrame(7, 10, true)).toBe(8);
  });

  it('runs the loaded first frame before any preview is visible', () => {
    expect(nextDngRunFrame(0, 3, false, null)).toBe(0);
  });

  it('resumes with the frame after the committed preview', () => {
    expect(nextDngRunFrame(0, 3, true, null)).toBe(1);
  });

  it('uses an asynchronously loaded pending frame when resuming', () => {
    expect(nextDngRunFrame(0, 3, true, 1)).toBe(1);
  });

  it('steps a prefetched frame with its matching descriptor and index', () => {
    const descriptor = { frameIndex: 1, fileName: 'frame-002.dng' };
    expect(dngFrameForStep({ index: 1, descriptor }, 0)).toEqual({ index: 1, descriptor });
  });

  it('steps the current frame when no prefetch is pending', () => {
    expect(dngFrameForStep(null, 4)).toEqual({ index: 4, descriptor: null });
  });

  it('accepts only the expected frame from the current sequence generation', () => {
    expect(isCurrentDngPrefetch({ index: 3, generation: 7 }, 3, 7)).toBe(true);
    expect(isCurrentDngPrefetch({ index: 2, generation: 7 }, 3, 7)).toBe(false);
    expect(isCurrentDngPrefetch({ index: 3, generation: 6 }, 3, 7)).toBe(false);
    expect(isCurrentDngPrefetch(null, 3, 7)).toBe(false);
  });

  it('matches a prefetch promise to its frame and sequence generation', () => {
    expect(isCurrentDngPrefetch({ index: 3, generation: 7 }, 3, 7)).toBe(true);
    expect(isCurrentDngPrefetch({ index: 3, generation: 7 }, 4, 7)).toBe(false);
  });
});
