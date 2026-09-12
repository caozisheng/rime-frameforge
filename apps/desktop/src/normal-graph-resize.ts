export interface GraphResizeScheduler {
  readonly ResizeObserver: new (callback: ResizeObserverCallback) => ResizeObserver;
  readonly requestAnimationFrame: (callback: FrameRequestCallback) => number;
  readonly cancelAnimationFrame: (handle: number) => void;
}

function defaultScheduler(): GraphResizeScheduler | null {
  if (typeof ResizeObserver === 'undefined' || typeof requestAnimationFrame === 'undefined' || typeof cancelAnimationFrame === 'undefined') return null;
  return {
    ResizeObserver,
    requestAnimationFrame: (callback) => globalThis.requestAnimationFrame(callback),
    cancelAnimationFrame: (handle) => globalThis.cancelAnimationFrame(handle),
  };
}

export function observeGraphResize(
  element: Element,
  onResize: () => void,
  scheduler: GraphResizeScheduler | null = defaultScheduler(),
): () => void {
  if (scheduler === null || scheduler === undefined) return () => {};

  let frame: number | null = null;
  const observer = new scheduler.ResizeObserver(() => {
    if (frame !== null) return;
    frame = scheduler.requestAnimationFrame(() => {
      frame = null;
      onResize();
    });
  });
  observer.observe(element);

  return () => {
    observer.disconnect();
    if (frame !== null) {
      scheduler.cancelAnimationFrame(frame);
      frame = null;
    }
  };
}
