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

function secant(lut: readonly number[], index: number): number {
  return lut[index + 1]! - lut[index]!;
}

function tangent(lut: readonly number[], index: number): number {
  if (index === 0) return secant(lut, 0);
  if (index >= 8) return secant(lut, 7);
  const left = secant(lut, index - 1);
  const right = secant(lut, index);
  return left * right <= 0 ? 0 : 2 * left * right / (left + right);
}

export function sampleGammaLuminanceLut(value: number, lut: readonly number[]): number {
  if (value > 1) return value;
  const coordinate = Math.max(0, Math.min(1, value)) * 8;
  const index = Math.min(Math.floor(coordinate), 7);
  const t = coordinate - index;
  const y0 = lut[index]!;
  const y1 = lut[index + 1]!;
  const control1 = y0 + tangent(lut, index) / 3;
  const control2 = y1 - tangent(lut, index + 1) / 3;
  const oneMinusT = 1 - t;
  const mapped = oneMinusT ** 3 * y0
    + 3 * oneMinusT ** 2 * t * control1
    + 3 * oneMinusT * t ** 2 * control2
    + t ** 3 * y1;
  return Math.max(Math.min(y0, y1), Math.min(Math.max(y0, y1), mapped));
}

export function applyGammaLuminanceCurve(rgb: readonly [number, number, number], parameters: GammaParameters): [number, number, number] {
  validateGammaParameters(parameters);
  const linear = rgb.map((value) => Math.max(0, value)) as [number, number, number];
  const luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
  if (luminance <= 1e-6) return [0, 0, 0];
  const gain = sampleGammaLuminanceLut(luminance, parameters.lut) / luminance;
  const inverseGamma = 1 / parameters.gamma;
  return linear.map((value) => (value * gain) ** inverseGamma) as [number, number, number];
}
