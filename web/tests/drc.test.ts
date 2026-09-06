import { describe, expect, it } from 'vitest';

import { generateGlobalToneLut } from '../src/gpu/drc.js';

describe('WebGPU DRC preprocessing', () => {
  it('generates an identity global LUT at unity gain', () => {
    const lut = generateGlobalToneLut(1);
    expect(lut).toHaveLength(257);
    lut.forEach((value, index) => expect(value).toBeCloseTo(index / 256, 5));
  });

  it('brightens midtones while preserving monotonic endpoints', () => {
    const lut = generateGlobalToneLut(2);
    expect(lut[0]).toBe(0);
    expect(lut[128]).toBeGreaterThan(0.5);
    expect(lut[256]).toBe(1);
    for (let index = 1; index < lut.length; index += 1) expect(lut[index]).toBeGreaterThanOrEqual(lut[index - 1]!);
  });

  it('rejects non-positive and non-finite gain', () => {
    expect(() => generateGlobalToneLut(0)).toThrow('DRC_TONE_INVALID');
    expect(() => generateGlobalToneLut(Number.NaN)).toThrow('DRC_TONE_INVALID');
  });
});
