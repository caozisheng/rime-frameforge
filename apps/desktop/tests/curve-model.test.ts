import { describe, expect, it } from 'vitest';
import { clampCurvePoint, interpolateCurve, moveCurvePoint, validateCurve } from '../src/components/iq/curve-model.js';

describe('IQ curve model', () => {
  const points = [{ x: -4, y: 0.1 }, { x: 0, y: 0 }, { x: 4, y: -0.1 }];

  it('moves only y while preserving fixed knots', () => {
    const moved = moveCurvePoint(points, 1, 0.25, { min: -1, max: 1 });
    expect(moved).toEqual([{ x: -4, y: 0.1 }, { x: 0, y: 0.25 }, { x: 4, y: -0.1 }]);
  });

  it('clamps points to the declared range', () => {
    expect(clampCurvePoint({ x: 0, y: 3 }, { min: -1, max: 1 })).toEqual({ x: 0, y: 1 });
    expect(clampCurvePoint({ x: 0, y: -3 }, { min: -1, max: 1 })).toEqual({ x: 0, y: -1 });
  });

  it('rejects duplicate knots and non-finite values', () => {
    expect(validateCurve([{ x: 0, y: 1 }, { x: 0, y: 2 }], { min: -1, max: 1 })).toEqual({ valid: false, reason: 'duplicate_or_unsorted_x' });
    expect(validateCurve([{ x: 0, y: Number.NaN }], { min: -1, max: 1 })).toEqual({ valid: false, reason: 'non_finite' });
  });

  it('interpolates direct LUT values by axis coordinate', () => {
    expect(interpolateCurve([{ x: 0, y: 2 }, { x: 4, y: 4 }], 2, 'linear')).toBe(3);
  });

  it('supports deterministic bezier fitting through LUT knots', () => {
    const points = [{ x: 0, y: 1 }, { x: 4, y: 3 }, { x: 8, y: 2 }];
    expect(interpolateCurve(points, 0, 'bezier')).toBe(1);
    expect(interpolateCurve(points, 4, 'bezier')).toBe(3);
    expect(interpolateCurve(points, 8, 'bezier')).toBe(2);
    expect(interpolateCurve(points, 2, 'bezier')).toBeGreaterThan(1);
  });

  it('keeps monotone Bézier segments within their neighboring knot values', () => {
    const monotone = [{ x: 0, y: 0 }, { x: 0.125, y: 0.6 }, { x: 0.25, y: 0.61 }, { x: 0.375, y: 0.9 }];
    for (let index = 0; index < monotone.length - 1; index += 1) {
      for (let sample = 0; sample <= 16; sample += 1) {
        const x = monotone[index]!.x + sample / 16 * (monotone[index + 1]!.x - monotone[index]!.x);
        const value = interpolateCurve(monotone, x, 'bezier');
        expect(value).toBeGreaterThanOrEqual(monotone[index]!.y);
        expect(value).toBeLessThanOrEqual(monotone[index + 1]!.y);
      }
    }
  });
});
