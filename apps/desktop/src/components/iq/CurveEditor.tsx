import { useId, type PointerEvent, type ReactNode } from 'react';

import { moveCurvePoint, type CurvePoint, type CurveRange } from './curve-model.js';

interface CurveEditorProps {
  readonly ariaLabel: string;
  readonly points: readonly CurvePoint[];
  readonly range: CurveRange;
  readonly disabled?: boolean;
  readonly onChange: (points: readonly CurvePoint[]) => void;
}

const WIDTH = 320;
const HEIGHT = 128;
const PADDING = 16;

export function CurveEditor({ ariaLabel, points, range, disabled = false, onChange }: CurveEditorProps): ReactNode {
  const titleId = useId();
  const minX = points[0]?.x ?? 0;
  const maxX = points.at(-1)?.x ?? 1;
  const toX = (value: number): number => PADDING + (value - minX) / Math.max(maxX - minX, Number.EPSILON) * (WIDTH - PADDING * 2);
  const toY = (value: number): number => HEIGHT - PADDING - (value - range.min) / Math.max(range.max - range.min, Number.EPSILON) * (HEIGHT - PADDING * 2);
  const fromPointerY = (event: PointerEvent<SVGCircleElement>): number => {
    const rect = event.currentTarget.ownerSVGElement?.getBoundingClientRect();
    if (rect === undefined) return 0;
    const y = (event.clientY - rect.top) / Math.max(rect.height, 1) * HEIGHT;
    return range.min + (HEIGHT - PADDING - y) / (HEIGHT - PADDING * 2) * (range.max - range.min);
  };
  const path = points
    .map((point, index) => `${index === 0 ? 'M' : 'L'} ${toX(point.x)} ${toY(point.y)}`)
    .join(' ');

  return (
    <div className="iq-curve-editor">
      <svg aria-labelledby={titleId} role="img" viewBox={`0 0 ${WIDTH} ${HEIGHT}`}>
        <title id={titleId}>{ariaLabel}</title>
        <path className="iq-curve-grid" d={`M ${PADDING} ${HEIGHT / 2} H ${WIDTH - PADDING}`} />
        <path className="iq-curve-path" d={path} />
        {points.map((point, index) => (
          <circle
            aria-label={`${ariaLabel} point ${index + 1}`}
            className="iq-curve-point"
            cx={toX(point.x)}
            cy={toY(point.y)}
            key={`${point.x}:${index}`}
            r="5"
            role="slider"
            tabIndex={disabled ? -1 : 0}
            onPointerDown={(event) => event.currentTarget.setPointerCapture(event.pointerId)}
            onPointerMove={(event) => {
              if (!disabled && event.currentTarget.hasPointerCapture(event.pointerId)) {
                onChange(moveCurvePoint(points, index, fromPointerY(event), range));
              }
            }}
            onPointerUp={(event) => event.currentTarget.releasePointerCapture(event.pointerId)}
          />
        ))}
      </svg>
      <div className="iq-curve-values">
        {points.map((point) => <span key={point.x}>{point.x}: {point.y.toFixed(2)}</span>)}
      </div>
    </div>
  );
}
