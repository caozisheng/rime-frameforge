import { useId, type PointerEvent, type ReactNode } from 'react';

import { interpolateCurve, moveCurvePoint, type CurveInterpolation, type CurvePoint, type CurveRange } from './curve-model.js';

interface CurveEditorProps {
  readonly ariaLabel: string;
  readonly points: readonly CurvePoint[];
  readonly range: CurveRange;
  readonly interpolation?: CurveInterpolation;
  readonly axisLabel?: string;
  readonly valueLabel?: string;
  readonly currentCoordinate?: number;
  readonly disabled?: boolean;
  readonly lockedPointIndices?: readonly number[];
  readonly onChange: (points: readonly CurvePoint[]) => void;
}

const WIDTH = 520;
const HEIGHT = 280;
const LEFT = 54;
const RIGHT = 16;
const TOP = 16;
const BOTTOM = 48;

export function CurveEditor({ ariaLabel, points, range, interpolation = 'linear', axisLabel = 'index', valueLabel = 'value', currentCoordinate, disabled = false, lockedPointIndices = [], onChange }: CurveEditorProps): ReactNode {
  const titleId = useId();
  const minX = points[0]?.x ?? 0;
  const maxX = points.at(-1)?.x ?? 1;
  const spanY = Math.max(range.max - range.min, Number.EPSILON);
  const toX = (value: number): number => LEFT + (value - minX) / Math.max(maxX - minX, Number.EPSILON) * (WIDTH - LEFT - RIGHT);
  const toY = (value: number): number => HEIGHT - BOTTOM - (value - range.min) / spanY * (HEIGHT - TOP - BOTTOM);
  const fromPointerY = (event: PointerEvent<SVGCircleElement>): number => {
    const rect = event.currentTarget.ownerSVGElement?.getBoundingClientRect();
    if (rect === undefined) return range.min;
    const y = (event.clientY - rect.top) / Math.max(rect.height, 1) * HEIGHT;
    return range.min + (HEIGHT - BOTTOM - y) / (HEIGHT - TOP - BOTTOM) * spanY;
  };
  const samples = interpolation === 'linear' ? points : Array.from({ length: 129 }, (_, index) => {
    const x = minX + index / 128 * (maxX - minX);
    return { x, y: interpolateCurve(points, x, interpolation) };
  });
  const path = samples.map((point, index) => `${index === 0 ? 'M' : 'L'} ${toX(point.x)} ${toY(point.y)}`).join(' ');
  const yTicks = Array.from({ length: 5 }, (_, index) => range.min + index / 4 * spanY);
  const markerValue = currentCoordinate === undefined ? undefined : interpolateCurve(points, currentCoordinate, interpolation);

  return <div className="iq-curve-editor">
    <svg aria-labelledby={titleId} role="img" viewBox={`0 0 ${WIDTH} ${HEIGHT}`}>
      <title id={titleId}>{ariaLabel}</title>
      {yTicks.map((tick) => <g key={tick}><line className="iq-curve-grid" x1={LEFT} x2={WIDTH - RIGHT} y1={toY(tick)} y2={toY(tick)} /><text className="iq-axis-tick" x={LEFT - 6} y={toY(tick) + 3} textAnchor="end">{tick.toFixed(2)}</text></g>)}
      {points.map((point, index) => <g key={`grid-${point.x}`}><line className="iq-curve-grid" x1={toX(point.x)} x2={toX(point.x)} y1={TOP} y2={HEIGHT - BOTTOM} /><text className="iq-axis-tick" x={toX(point.x)} y={HEIGHT - BOTTOM + 15} textAnchor="middle">{index}</text><text className="iq-axis-knot" x={toX(point.x)} y={HEIGHT - BOTTOM + 29} textAnchor="middle">{point.x}</text></g>)}
      <text className="iq-axis-label" x={(LEFT + WIDTH - RIGHT) / 2} y={HEIGHT - 5} textAnchor="middle">{axisLabel}</text>
      <text className="iq-axis-label" transform={`translate(12 ${(TOP + HEIGHT - BOTTOM) / 2}) rotate(-90)`} textAnchor="middle">{valueLabel}</text>
      <path className="iq-curve-path" d={path} />
      {currentCoordinate === undefined || markerValue === undefined ? null : <g className="iq-current-marker"><line x1={toX(currentCoordinate)} x2={toX(currentCoordinate)} y1={TOP} y2={HEIGHT - BOTTOM} /><circle cx={toX(currentCoordinate)} cy={toY(markerValue)} r="4" /></g>}
      {points.map((point, index) => { const locked = disabled || lockedPointIndices.includes(index); return <circle aria-disabled={locked} aria-label={`${ariaLabel} point ${index + 1}`} aria-valuemax={range.max} aria-valuemin={range.min} aria-valuenow={point.y} className="iq-curve-point" cx={toX(point.x)} cy={toY(point.y)} key={`${point.x}:${index}`} r="5" role="slider" tabIndex={locked ? -1 : 0} onPointerDown={(event) => { if (!locked) event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { if (!locked && event.currentTarget.hasPointerCapture(event.pointerId)) onChange(moveCurvePoint(points, index, fromPointerY(event), range)); }} onPointerUp={(event) => { if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }} />; })}
    </svg>
    <div className="iq-curve-values">{points.map((point, index) => <span key={point.x}>[{index}] {point.x}: {point.y.toFixed(4)}</span>)}</div>
  </div>;
}
