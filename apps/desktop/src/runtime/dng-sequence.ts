export function nextDngSequenceFrame(currentIndex: number, frameCount: number, playing: boolean): number | null {
  if (!playing || currentIndex + 1 >= frameCount) return null;
  return currentIndex + 1;
}

export function nextDngRunFrame(currentIndex: number, frameCount: number, visibleFrameCommitted: boolean, pendingIndex: number | null): number {
  if (pendingIndex !== null && pendingIndex > currentIndex) return pendingIndex;
  if (visibleFrameCommitted && currentIndex + 1 < frameCount) return currentIndex + 1;
  return currentIndex;
}

export function dngFrameForStep<T>(pending: { readonly index: number; readonly descriptor: T } | null, currentIndex: number): { readonly index: number; readonly descriptor: T | null } {
  return pending === null ? { index: currentIndex, descriptor: null } : pending;
}

export function canLoadNextDngFrame(lifecycleState: string): boolean {
  return lifecycleState === 'stop' || lifecycleState === 'completed';
}

export function shouldResumeDngSequence(playing: boolean, loadedFrameIndex: number, expectedFrameIndex: number): boolean {
  return playing && loadedFrameIndex === expectedFrameIndex;
}

export const shouldCommitDngDescriptor = shouldResumeDngSequence;

export function visibleDngFrameIndex(currentIndex: number, nextIndex: number, committed: boolean): number {
  return committed ? nextIndex : currentIndex;
}

export function commitPendingDngFrame<T>(pending: { readonly index: number; readonly descriptor: T } | null, currentIndex: number): { readonly index: number; readonly descriptor: T } | null {
  return pending !== null && pending.index > currentIndex ? pending : null;
}
