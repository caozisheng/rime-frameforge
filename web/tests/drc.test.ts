import { describe, expect, it } from 'vitest';

import type { BayerCfa, RawFrameDescriptor } from '../src/contracts.js';
import { DEFAULT_DRC_IQ_PARAMETERS, generateGlobalToneLut, packDrcUniforms, validateDrcIqParameters } from '../src/gpu/drc.js';

const descriptor: Pick<RawFrameDescriptor, 'baselineExposure' | 'whiteBalanceGains' | 'cfa'> = { baselineExposure: 1.5, whiteBalanceGains: [2, 1, 4], cfa: 'rggb' satisfies BayerCfa };

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

  it('validates DRC IQ parameters and packs final gain from metadata exposure and offset', () => {
    expect(DEFAULT_DRC_IQ_PARAMETERS).toEqual({ drc_gain_offset_ev: 0, knee: 1, amplifier: 3 });
    validateDrcIqParameters({ drc_gain_offset_ev: 2, knee: 0.5, amplifier: 0 });
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 4.1, knee: 1, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 0, knee: 0, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 0, knee: 1, amplifier: -1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: Number.NaN, knee: 1, amplifier: 1 })).toThrow('DRC_IQ_INVALID');

    const packed = new DataView(packDrcUniforms(descriptor, { drc_gain_offset_ev: 0.5, knee: 0.25, amplifier: 2 }));
    expect(packed.getFloat32(0, true)).toBeCloseTo(2 ** 2);
    expect(packed.getFloat32(4, true)).toBeCloseTo(0.25);
    expect(packed.getFloat32(8, true)).toBeCloseTo(2);
  });

  it('packs no white-balance fields; DRC consumes the balanced input', () => {
    const packed = new DataView(packDrcUniforms(descriptor, DEFAULT_DRC_IQ_PARAMETERS));
    expect(packed.byteLength).toBe(32);
  });
});
