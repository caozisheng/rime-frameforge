import { Position, type Node } from '@xyflow/react';
import { getSmartEdge } from '@tisoap/react-flow-smart-edge';
import { describe, expect, it } from 'vitest';

describe('smart edge routing', () => {
  it('routes around a measured third-party node rectangle', () => {
    const nodes: Node[] = [
      { id: 'source', position: { x: 0, y: 60 }, measured: { width: 100, height: 60 }, data: {} },
      { id: 'blocker', position: { x: 140, y: 40 }, measured: { width: 100, height: 100 }, data: {} },
      { id: 'target', position: { x: 300, y: 60 }, measured: { width: 100, height: 60 }, data: {} },
    ];
    const result = getSmartEdge({
      nodes,
      sourceX: 100,
      sourceY: 90,
      sourcePosition: Position.Right,
      targetX: 300,
      targetY: 90,
      targetPosition: Position.Left,
      options: { gridRatio: 4, nodePadding: 10 },
    });

    expect(result).not.toBeInstanceOf(Error);
    if (result instanceof Error) throw result;
    expect(result.points.some(([, y]) => y < 30 || y > 150)).toBe(true);
    expect(result.points.every(([x, y]) => !(x > 130 && x < 250 && y > 30 && y < 150))).toBe(true);
  });

  it('routes a stage edge around an unrelated measured first-level frame', () => {
    const nodes: Node[] = [
      { id: 'vfe', position: { x: 0, y: 0 }, measured: { width: 220, height: 180 }, data: {} },
      { id: 'vpe', position: { x: 260, y: 100 }, measured: { width: 260, height: 260 }, data: {} },
      { id: 'vbe', position: { x: 600, y: 0 }, measured: { width: 220, height: 180 }, data: {} },
    ];
    const result = getSmartEdge({
      nodes,
      sourceX: 220,
      sourceY: 90,
      sourcePosition: Position.Right,
      targetX: 600,
      targetY: 90,
      targetPosition: Position.Left,
      options: { gridRatio: 4, nodePadding: 12 },
    });

    expect(result).not.toBeInstanceOf(Error);
    if (result instanceof Error) throw result;
    expect(result.points.every(([x, y]) => !(x > 248 && x < 532 && y > 88 && y < 372))).toBe(true);
  });
});
