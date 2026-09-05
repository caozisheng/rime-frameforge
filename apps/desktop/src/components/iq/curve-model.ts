export interface CurvePoint {
  readonly x: number;
  readonly y: number;
}

export interface CurveRange {
  readonly min: number;
  readonly max: number;
}
export type CurveInterpolation = 'linear' | 'bezier';

export function interpolateCurve(points: readonly CurvePoint[], x: number, interpolation: CurveInterpolation = 'linear'): number {
  if (points.length === 0) return Number.NaN;
  if (points.length === 1) return points[0]!.y;
  const coordinate = Math.min(points.at(-1)!.x, Math.max(points[0]!.x, x));
  const index = points.findIndex((point, pointIndex) => pointIndex > 0 && coordinate <= point.x);
  const rightIndex = index < 0 ? points.length - 1 : index;
  const leftIndex = rightIndex - 1;
  const left = points[leftIndex]!;
  const right = points[rightIndex]!;
  const fraction = (coordinate - left.x) / (right.x - left.x);
  if (interpolation === 'linear') return left.y + fraction * (right.y - left.y);
  const delta = (segment: number): number => points[segment + 1]!.y - points[segment]!.y;
  const tangent = (pointIndex: number): number => {
    if (pointIndex === 0) return delta(0);
    if (pointIndex >= points.length - 1) return delta(points.length - 2);
    const leftDelta = delta(pointIndex - 1);
    const rightDelta = delta(pointIndex);
    return leftDelta * rightDelta <= 0 ? 0 : 2 * leftDelta * rightDelta / (leftDelta + rightDelta);
  };
  const p0 = left.y;
  const p1 = left.y + tangent(leftIndex) / 3;
  const p2 = right.y - tangent(rightIndex) / 3;
  const p3 = right.y;
  const inverse = 1 - fraction;
  const value = inverse ** 3 * p0 + 3 * inverse ** 2 * fraction * p1 + 3 * inverse * fraction ** 2 * p2 + fraction ** 3 * p3;
  return Math.max(Math.min(p0, p3), Math.min(Math.max(p0, p3), value));
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
