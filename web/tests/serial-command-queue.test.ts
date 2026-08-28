import { describe, expect, it } from 'vitest';

import { SerialCommandQueue } from '../src/serial-command-queue.js';

describe('SerialCommandQueue', () => {
  it('does not start reset before an in-flight step completes', async () => {
    const events: string[] = [];
    let finishStep: (() => void) | undefined;
    const stepDone = new Promise<void>((resolve) => {
      finishStep = resolve;
    });
    const queue = new SerialCommandQueue();

    const step = queue.enqueue(async () => {
      events.push('step:start');
      await stepDone;
      events.push('step:end');
    });
    const reset = queue.enqueue(() => {
      events.push('reset');
    });
    await Promise.resolve();

    expect(events).toEqual(['step:start']);
    finishStep?.();
    await Promise.all([step, reset]);
    expect(events).toEqual(['step:start', 'step:end', 'reset']);
  });

  it('continues after a rejected command', async () => {
    const events: string[] = [];
    const queue = new SerialCommandQueue();

    await expect(queue.enqueue(() => Promise.reject(new Error('failed')))).rejects.toThrow('failed');
    await queue.enqueue(() => events.push('next'));

    expect(events).toEqual(['next']);
  });
});
