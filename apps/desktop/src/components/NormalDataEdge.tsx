import { BaseEdge, getSmoothStepPath, type EdgeProps } from '@xyflow/react';
import { useSmartEdgePath } from '@tisoap/react-flow-smart-edge';

import { getNormalDataArrowPoints, getNormalDataArrowPointsFromSegment } from '../normal-edge.js';

export function NormalDataEdge(props: EdgeProps) {
  const {
    id,
    source,
    target,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    label,
    labelStyle,
    labelBgStyle,
    labelBgPadding,
    pathOptions,
    style,
  } = props;
  const { route } = useSmartEdgePath({
    id,
    source,
    target,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    preset: 'smoothstep',
    options: { gridRatio: 6, nodePadding: 12, borderRadius: 10 },
  });
  const options = pathOptions as { offset?: number; borderRadius?: number } | undefined;
  const [fallbackPath, fallbackLabelX, fallbackLabelY] = getSmoothStepPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    offset: options?.offset ?? 42,
    borderRadius: options?.borderRadius ?? 10,
  });
  const routed = route?.kind === 'routed' ? route : null;
  const path = routed?.svgPathString ?? fallbackPath;
  const bend = routed?.points.find((point, index, points) => {
    const previous = points[index - 1];
    const next = points[index + 1];
    if (previous === undefined || next === undefined) return false;
    const pointX = point[0];
    const pointY = point[1];
    const previousX = previous[0];
    const previousY = previous[1];
    const nextX = next[0];
    const nextY = next[1];
    if (pointX === undefined || pointY === undefined || previousX === undefined || previousY === undefined || nextX === undefined || nextY === undefined) return false;
    return (pointX - previousX) * (nextY - pointY) !== (pointY - previousY) * (nextX - pointX);
  });
  const labelX = bend?.[0] ?? routed?.edgeCenterX ?? fallbackLabelX;
  const labelY = bend?.[1] ?? routed?.edgeCenterY ?? fallbackLabelY;
  const previous = routed?.points.at(-1);
  const points = previous === undefined
    ? getNormalDataArrowPoints(targetX, targetY, targetPosition)
    : getNormalDataArrowPointsFromSegment(targetX, targetY, previous[0] ?? sourceX, previous[1] ?? sourceY);

  return (
    <>
      <BaseEdge
        id={id}
        path={path}
        labelX={labelX}
        labelY={labelY}
        label={label}
        {...(labelStyle === undefined ? {} : { labelStyle })}
        {...(labelBgStyle === undefined ? {} : { labelBgStyle })}
        {...(labelBgPadding === undefined ? {} : { labelBgPadding })}
        {...(style === undefined ? {} : { style })}
      />
      <polygon className="normal-data-arrow" data-edge-arrow={id} data-smart-routed={routed !== null} points={points} />
    </>
  );
}
