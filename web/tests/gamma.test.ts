import { describe, expect, it } from 'vitest';

import { applyGammaLuminanceCurve, DEFAULT_GAMMA_PARAMETERS, validateGammaParameters } from '../src/gpu/gamma.js';

describe('Gamma luminance curve contract', () => {
  it('leaves linear luminance unchanged with the identity LUT before gamma encoding', () => {
    const encoded = applyGammaLuminanceCurve([0.18, 0.09, 0.045], DEFAULT_GAMMA_PARAMETERS);
    expect(encoded[0]).toBeCloseTo(0.18 ** (1 / 2.2), 12);
    expect(encoded[1]).toBeCloseTo(0.09 ** (1 / 2.2), 12);
    expect(encoded[2]).toBeCloseTo(0.045 ** (1 / 2.2), 12);
  });

  it('applies one luminance gain to all RGB channels before gamma encoding', () => {
    const linear = [0.4, 0.2, 0.1] as const;
    const parameters = { gamma: 2.0, lut: [0, 0.2, 0.35, 0.48, 0.6, 0.7, 0.8, 0.9, 1] } as const;
    const encoded = applyGammaLuminanceCurve(linear, parameters);
    const restored = encoded.map((value) => value ** parameters.gamma);
    expect(restored[0]! / restored[1]!).toBeCloseTo(linear[0] / linear[1], 6);
    expect(restored[1]! / restored[2]!).toBeCloseTo(linear[1] / linear[2], 6);
  });

  it('accepts gamma 1.8 through 2.4 in 0.1 increments and rejects invalid LUTs', () => {
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 1.8 })).not.toThrow();
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 2.4 })).not.toThrow();
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, gamma: 1.85 })).toThrow('GAMMA_PARAMETER_INVALID');
    expect(() => validateGammaParameters({ ...DEFAULT_GAMMA_PARAMETERS, lut: [0, 0.2, 0.1, 0.4, 0.5, 0.6, 0.7, 0.8, 1] })).toThrow('GAMMA_LUT_INVALID');
  });
});
