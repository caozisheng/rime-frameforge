import { describe, expect, it } from 'vitest';

import {
  GpuCapabilityError,
  estimateNormalGraphLivePeakBytes,
  estimateNormalGraphPoolBytes,
  validateGpuInput,
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

  it('estimates the serial live working set', () => {
    expect(estimateNormalGraphLivePeakBytes(gh5s)).toBe(3744 * 2776 * (2 + 16 + 16));
  });

  it('estimates the retained cold-start pool separately', () => {
    expect(estimateNormalGraphPoolBytes(gh5s)).toBe(3744 * 2776 * (2 + 4 + 4 + 16 + 16 + 16 + 16));
  });
});
