export interface CurvePoint {
  readonly x: number;
  readonly y: number;
}

export interface CurveRange {
  readonly min: number;
  readonly max: number;
}

export type CurveValidation = { readonly valid: true } | { readonly valid: false; readonly reason: 'duplicate_or_unsorted_x' | 'non_finite' | 'out_of_range' };

export function clampCurvePoint(point: CurvePoint, range: CurveRange): CurvePoint {
  return { x: point.x, y: Math.min(range.max, Math.max(range.min, point.y)) };
}

export function moveCurvePoint(points: readonly CurvePoint[], index: number, y: number, range: CurveRange): readonly CurvePoint[] {
  if (index < 0 || index >= points.length) return points;
  return points.map((point, pointIndex) => pointIndex === index ? clampCurvePoint({ x: point.x, y }, range) : point);
}

export function validateCurve(points: readonly CurvePoint[], range: CurveRange): CurveValidation {
  if (points.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))) return { valid: false, reason: 'non_finite' };
  if (points.some((point, index) => index > 0 && points[index - 1]!.x >= point.x)) return { valid: false, reason: 'duplicate_or_unsorted_x' };
  if (points.some((point) => point.y < range.min || point.y > range.max)) return { valid: false, reason: 'out_of_range' };
  return { valid: true };
}
