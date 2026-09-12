import { afterEach, describe, expect, it, vi } from 'vitest';

import { observeGraphResize, type GraphResizeScheduler } from '../src/normal-graph-resize.js';

class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];
  readonly callback: ResizeObserverCallback;
  disconnected = false;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    FakeResizeObserver.instances.push(this);
  }

  observe(_target: Element): void {}
  unobserve(_target: Element): void {}
  disconnect(): void {
    this.disconnected = true;
  }

  resize(): void {
    this.callback([], this);
  }
}

function fakeScheduler() {
  let nextFrame = 1;
  const frames = new Map<number, FrameRequestCallback>();
  const scheduler: GraphResizeScheduler = {
    ResizeObserver: FakeResizeObserver,
    requestAnimationFrame: (callback) => {
      const handle = nextFrame++;
      frames.set(handle, callback);
      return handle;
    },
    cancelAnimationFrame: (handle) => {
      frames.delete(handle);
    },
  };
  return {
    scheduler,
    flushFrames: () => {
      const pending = [...frames.values()];
      frames.clear();
      pending.forEach((callback) => callback(0));
    },
    pendingFrameCount: () => frames.size,
  };
}
afterEach(() => {
  vi.unstubAllGlobals();
});


describe('observeGraphResize', () => {
  it('coalesces resize notifications until the next animation frame', () => {
    FakeResizeObserver.instances = [];
    const { scheduler, flushFrames } = fakeScheduler();
    let fitCount = 0;
    observeGraphResize({} as Element, () => { fitCount += 1; }, scheduler);
    const observer = FakeResizeObserver.instances[0];

    observer.resize();
    observer.resize();
    expect(fitCount).toBe(0);

    flushFrames();
    expect(fitCount).toBe(1);
  });

  it('disconnects and cancels pending fitting during cleanup', () => {
    FakeResizeObserver.instances = [];
    const { scheduler, flushFrames, pendingFrameCount } = fakeScheduler();
    let fitCount = 0;
    const cleanup = observeGraphResize({} as Element, () => { fitCount += 1; }, scheduler);
    const observer = FakeResizeObserver.instances[0];

    observer.resize();
    cleanup();
    flushFrames();

    expect(observer.disconnected).toBe(true);
    expect(pendingFrameCount()).toBe(0);
    expect(fitCount).toBe(0);
  });

  it('preserves the browser receiver when scheduling and cancelling frames', () => {
    FakeResizeObserver.instances = [];
    let requestReceiver: unknown;
    let cancelReceiver: unknown;
    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
    vi.stubGlobal('requestAnimationFrame', function (this: unknown, _callback: FrameRequestCallback): number {
      requestReceiver = this;
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', function (this: unknown, _handle: number): void {
      cancelReceiver = this;
    });

    const cleanup = observeGraphResize({} as Element, () => {});
    FakeResizeObserver.instances[0].resize();
    cleanup();

    expect(requestReceiver).toBe(globalThis);
    expect(cancelReceiver).toBe(globalThis);
  });
});
