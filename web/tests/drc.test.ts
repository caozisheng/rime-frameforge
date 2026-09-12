import { describe, expect, it } from 'vitest';

import { DEFAULT_DRC_IQ_PARAMETERS, validateDrcIqParameters } from '../src/gpu/drc.js';

describe('WebGPU DRC configuration', () => {
  it('validates DRC IQ parameters before Rust packet derivation', () => {
    expect(DEFAULT_DRC_IQ_PARAMETERS).toEqual({ drc_gain_offset_ev: 0, knee: 1, amplifier: 3 });
    validateDrcIqParameters({ drc_gain_offset_ev: 2, knee: 0.5, amplifier: 0 });
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 4.1, knee: 1, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 0, knee: 0, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: 0, knee: 1, amplifier: -1 })).toThrow('DRC_IQ_INVALID');
    expect(() => validateDrcIqParameters({ drc_gain_offset_ev: Number.NaN, knee: 1, amplifier: 1 })).toThrow('DRC_IQ_INVALID');
  });
});

