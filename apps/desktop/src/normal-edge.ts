import { MarkerType, Position, type Edge } from '@xyflow/react';

import type { VisibleNormalEdge } from '../../../web/src/normal-flow.js';

export const normalHandlePositions = {
  target: Position.Left,
  source: Position.Right,
} as const;

export interface NormalPortHandle {
  readonly id: string;
  readonly top: string;
}

export function getNormalPortHandles(portIds: readonly string[]): NormalPortHandle[] {
  return portIds.map((id, index) => ({
    id,
    top: `${((index + 1) / (portIds.length + 1)) * 100}%`,
  }));
}

export type NormalReactFlowEdge = Edge & {
  readonly pathOptions: { readonly offset: number; readonly borderRadius: number };
  readonly labelPosition?: 'bend';
};

export function toNormalReactFlowEdge(edge: VisibleNormalEdge): NormalReactFlowEdge {
  const isDataEdge = edge.label !== undefined;
  return {
    ...edge,
    sourceHandle: edge.sourcePort,
    targetHandle: edge.targetPort,
    type: isDataEdge ? 'normalData' : 'smartSmooth',
    label: edge.label,
    ...(isDataEdge ? { labelPosition: 'bend' as const } : {}),
    zIndex: 2,
    style: { stroke: '#8291a0', strokeWidth: 1.5 },
    labelStyle: { fill: '#536270', fontSize: 10, fontWeight: isDataEdge ? 600 : 400 },
    labelBgStyle: { fill: '#ffffff', fillOpacity: 0.94 },
    labelBgPadding: [5, 3],
    pathOptions: { offset: 28, borderRadius: 10 },
    ...(isDataEdge ? {} : {
      markerEnd: {
        type: MarkerType.ArrowClosed,
        color: '#8291a0',
        width: 13,
        height: 13,
      },
    }),
  };
}

export function getNormalDataArrowPoints(
  x: number,
  y: number,
  position: 'left' | 'right' | 'top' | 'bottom',
): string {
  const length = 13;
  const half = 6;
  if (position === 'right') return `${x},${y} ${x + length},${y - half} ${x + length},${y + half}`;
  if (position === 'top') return `${x},${y} ${x - half},${y - length} ${x + half},${y - length}`;
  if (position === 'bottom') return `${x},${y} ${x - half},${y + length} ${x + half},${y + length}`;
  return `${x},${y} ${x - length},${y - half} ${x - length},${y + half}`;
}

export function getNormalDataArrowPointsFromSegment(
  targetX: number,
  targetY: number,
  previousX: number,
  previousY: number,
): string {
  const dx = targetX - previousX;
  const dy = targetY - previousY;
  if (Math.abs(dx) >= Math.abs(dy)) {
    return getNormalDataArrowPoints(targetX, targetY, dx >= 0 ? 'left' : 'right');
  }
  return getNormalDataArrowPoints(targetX, targetY, dy >= 0 ? 'top' : 'bottom');
}
