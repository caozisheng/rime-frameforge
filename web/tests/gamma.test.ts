import { describe, expect, it } from 'vitest';

import { DEFAULT_GAMMA_PARAMETERS, validateGammaParameters } from '../src/gpu/gamma.js';

describe('Gamma configuration', () => {
  it('validates input before Rust packet derivation', () => {
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 1.8 })).not.toThrow();
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 2.4 })).not.toThrow();
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 1.85 })).toThrow('GAMMA_PARAMETER_INVALID');
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, lut: [0, 0.2, 0.1, 0.4, 0.5, 0.6, 0.7, 0.8, 1] })).toThrow('GAMMA_LUT_INVALID');
  });
});
