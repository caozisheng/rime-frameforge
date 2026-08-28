import { describe, expect, it } from 'vitest';

import { GpuTexturePool, StaleTextureLease } from '../src/gpu/texture-pool.js';
import type { TextureKey } from '../src/gpu/texture-pool.js';
const key: TextureKey = {
  domain: 'linear_rgb',
  format: 'rgba32float',
  width: 64,
  height: 48,
  usage: (1 << 7) | (1 << 2),
};

function texture(id: string): GPUTexture {
  return { label: id, destroy: () => undefined } as unknown as GPUTexture;
}

describe('GpuTexturePool', () => {
  it('reuses a released texture with the same key', () => {
    const pool = new GpuTexturePool(1);
    const first = pool.acquire(key, () => texture('first'));
    pool.release(first);
    const second = pool.acquire(key, () => texture('second'));

    expect(second.resourceId).toBe(first.resourceId);
    expect(second.texture).toBe(first.texture);
  });

  it('does not reuse a texture with a different extent', () => {
    const pool = new GpuTexturePool(1);
    const first = pool.acquire(key, () => texture('first'));
    pool.release(first);
    const second = pool.acquire({ ...key, width: 128 }, () => texture('second'));

    expect(second.resourceId).not.toBe(first.resourceId);
  });

  it('does not reuse a texture across signal domains', () => {
    const pool = new GpuTexturePool(1);
    const first = pool.acquire(key, () => texture('first'));
    pool.release(first);
    const second = pool.acquire({ ...key, domain: 'yuv' }, () => texture('second'));

    expect(second.resourceId).not.toBe(first.resourceId);
  });

  it('rejects a lease after generation invalidation', () => {
    const pool = new GpuTexturePool(1);
    const lease = pool.acquire(key, () => texture('first'));
    pool.invalidateGeneration(2);

    expect(() => pool.resolve(lease)).toThrow(StaleTextureLease);
  });

  it('does not return a live lease to the free list twice', () => {
    const pool = new GpuTexturePool(1);
    const lease = pool.acquire(key, () => texture('first'));

    pool.release(lease);
    expect(() => pool.release(lease)).toThrow(StaleTextureLease);
  });

  it('reuses pooled storage after generation reset without destroying it', () => {
    const pool = new GpuTexturePool(1);
    const first = pool.acquire(key, () => texture('first'));
    pool.release(first);
    pool.invalidateGeneration(2);
    const second = pool.acquire(key, () => texture('second'));

    expect(second.texture).toBe(first.texture);
    expect(second.generation).toBe(2);
  });
});
