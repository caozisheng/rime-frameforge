import { describe, expect, it } from 'vitest';

import { GpuResourceRegistry, StaleGpuResource } from '../src/gpu/resource-registry.js';

describe('GpuResourceRegistry', () => {
  it('resolves a resource from the current generation', () => {
    const registry = new GpuResourceRegistry(3);
    const texture = { destroy: () => undefined } as unknown as GPUTexture;
    const reference = registry.register('wbc.out', texture);

    expect(registry.resolve(reference)).toBe(texture);
  });

  it('rejects a resource after generation invalidation', () => {
    const registry = new GpuResourceRegistry(3);
    const reference = registry.register(
      'wbc.out',
      { destroy: () => undefined } as unknown as GPUTexture,
    );
    registry.invalidate(4);

    expect(() => registry.resolve(reference)).toThrow(StaleGpuResource);
  });

  it('destroys every registered resource on invalidation', () => {
    let destroyed = 0;
    const registry = new GpuResourceRegistry(3);
    registry.register('wbc.out', { destroy: () => destroyed++ } as unknown as GPUTexture);

    registry.invalidate(4);

    expect(destroyed).toBe(1);
  });

  it('preserves immutable input textures across generation invalidation', () => {
    let destroyed = 0;
    const texture = { destroy: () => destroyed++ } as unknown as GPUTexture;
    const registry = new GpuResourceRegistry(3);
    registry.register('raw_source.out', texture, { destroyOnInvalidate: false });

    registry.invalidate(4);
    const rebound = registry.register('raw_source.out', texture, { destroyOnInvalidate: false });

    expect(registry.resolve(rebound)).toBe(texture);
    expect(destroyed).toBe(0);
  });

  it('releases a transient texture exactly once', () => {
    let destroyed = 0;
    const registry = new GpuResourceRegistry(3);
    const reference = registry.register(
      'gamma.out',
      { destroy: () => destroyed++ } as unknown as GPUTexture,
    );

    registry.release(reference);
    registry.invalidate(4);

    expect(destroyed).toBe(1);
  });
});
