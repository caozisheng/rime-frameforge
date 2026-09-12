export interface GammaParameters {
  readonly gamma: number;
  readonly lut: readonly number[];
}

export const DEFAULT_GAMMA_PARAMETERS: GammaParameters = Object.freeze({
  gamma: 2.2,
  lut: Object.freeze([0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1]),
});

export function validateGammaParameters(parameters: GammaParameters): void {
  const gammaStep = parameters.gamma * 10;
  if (!Number.isFinite(parameters.gamma) || parameters.gamma < 1.8 || parameters.gamma > 2.4 || Math.abs(gammaStep - Math.round(gammaStep)) > 1e-6) {
    throw new Error('GAMMA_PARAMETER_INVALID: gamma must be 1.8 through 2.4 in 0.1 increments');
  }
  if (parameters.lut.length !== 9 || parameters.lut[0] !== 0 || parameters.lut[8] !== 1) {
    throw new Error('GAMMA_LUT_INVALID: luminance LUT requires nine points with fixed endpoints');
  }
  for (let index = 0; index < parameters.lut.length; index += 1) {
    const value = parameters.lut[index]!;
    if (!Number.isFinite(value) || value < 0 || value > 1 || (index > 0 && value < parameters.lut[index - 1]!)) {
      throw new Error('GAMMA_LUT_INVALID: luminance LUT must be finite, bounded, and monotone');
    }
  }
}

