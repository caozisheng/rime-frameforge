import { describe, expect, it } from 'vitest';

import {
  GpuCapabilityError,
  estimateNormalGraphLivePeakBytes,
  estimateNormalGraphPoolBytes,
  normalGraphRequiredLimits,
  validateGpuInput,
  validateNormalGraphAdapterLimits,
} from '../src/gpu/capability.js';
import type { RawFrameDescriptor } from '../src/contracts.js';

const gh5s: RawFrameDescriptor = {
  width: 3744,
  height: 2776,
  rowStrideSamples: 3744,
  storageBits: 16,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 65535,
  whiteBalanceGains: [2, 1, 1.5],
  metadata: { colorMatrix1: [1, 0, 0, 0, 1, 0, 0, 0, 1] },
};

describe('validateGpuInput', () => {
  it('accepts the GH5S extent within the declared budget', () => {
    expect(() => validateGpuInput(gh5s, 8192)).not.toThrow();
  });

  it('rejects an extent beyond the device texture limit', () => {
    expect(() => validateGpuInput({ ...gh5s, width: 9000 }, 8192)).toThrow(GpuCapabilityError);
  });

  it('rejects a graph allocation beyond the memory budget', () => {
    expect(() => validateGpuInput({ ...gh5s, width: 8000, height: 8000 }, 4096)).toThrow(
      GpuCapabilityError,
    );
  });

  it('accepts a 24MP frame after compacting DRC intermediate formats', () => {
    expect(() => validateGpuInput({ ...gh5s, width: 6000, height: 4096, rowStrideSamples: 6000 }, 4096)).not.toThrow();
  });

  it('accounts for retained Normal Graph and three-level DRC resources', () => {
    const full = 3744 * 2776;
    const half = Math.floor(3744 / 2) * Math.floor(2776 / 2);
    const quarter = Math.floor(3744 / 4) * Math.floor(2776 / 4);
    const expected = full * 54 + (full + half + quarter) * 44 + 48 + 257 * 49 * 4;
    expect(estimateNormalGraphLivePeakBytes(gh5s)).toBe(expected);
    expect(estimateNormalGraphPoolBytes(gh5s)).toBe(expected);
  });

  it('requests the six storage textures used by the fused Preview pipeline', () => {
    expect(normalGraphRequiredLimits()).toEqual({ maxStorageTexturesPerShaderStage: 6 });
  });

  it('rejects adapters that cannot bind all Preview stage outputs', () => {
    expect(() => validateNormalGraphAdapterLimits({ maxStorageTexturesPerShaderStage: 4 })).toThrow('GPU_CAPABILITY_UNSUPPORTED');
    expect(() => validateNormalGraphAdapterLimits({ maxStorageTexturesPerShaderStage: 8 })).not.toThrow();
  });
});
