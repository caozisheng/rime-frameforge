import { Position } from '@xyflow/react';

import { describe, expect, it } from 'vitest';

import {
  getNormalDataArrowPoints,
  getNormalDataArrowPointsFromSegment,
  getNormalPortHandles,

  normalHandlePositions,
  toNormalReactFlowEdge,
} from '../src/normal-edge.js';

describe('toNormalReactFlowEdge', () => {
  it('keeps labeled VBE to VPE data visually consistent with ordinary edges', () => {
    const dataEdge = toNormalReactFlowEdge({
      id: 'vbe-vpe',
      source: 'vbe',
      target: 'vpe',
      label: 'Full YUV / 1/4 YUV / 1/16 YUV',
    });
    const ordinaryEdge = toNormalReactFlowEdge({ id: 'ordinary', source: 'a', target: 'b' });

    expect(dataEdge.type).toBe('normalData');
    expect(dataEdge.zIndex).toBe(ordinaryEdge.zIndex);
    expect(dataEdge.style).toEqual(ordinaryEdge.style);
    expect(dataEdge.markerEnd).toBeUndefined();
    expect(dataEdge.pathOptions).toEqual(ordinaryEdge.pathOptions);
  });

  it('uses the smart router for ordinary edges', () => {
    const edge = toNormalReactFlowEdge({ id: 'ordinary', source: 'a', target: 'b' });
    expect(edge.type).toBe('smartSmooth');
    expect(edge.pathOptions).toMatchObject({ offset: 28, borderRadius: 10 });
  });

  it('keeps every input on the left and every output on the right', () => {
    expect(normalHandlePositions).toEqual({ target: Position.Left, source: Position.Right });
  });

  it('evenly spaces every port while keeping stable ids', () => {
    expect(getNormalPortHandles([])).toEqual([]);
    expect(getNormalPortHandles(['in'])).toEqual([{ id: 'in', top: '50%' }]);
    expect(getNormalPortHandles(['full', 'quarter', 'sixteenth'])).toEqual([
      { id: 'full', top: '25%' },
      { id: 'quarter', top: '50%' },
      { id: 'sixteenth', top: '75%' },
    ]);
  });

  it('binds each React Flow edge to its concrete handles', () => {
    const edge = toNormalReactFlowEdge({
      id: 'multi-port',
      source: 'pyrd',
      sourcePort: 'quarter',
      target: 'vpe_4_pyrc',
      targetPort: 'in',
      label: '1/4 YUV',
    });

    expect(edge.sourceHandle).toBe('quarter');
    expect(edge.targetHandle).toBe('in');
  });

  it('marks every labeled scale edge for bend-positioned labels', () => {
    for (const label of ['Full YUV', '1/4 YUV', '1/16 YUV']) {
      const edge = toNormalReactFlowEdge({
        id: label,
        source: 'pyrd',
        sourcePort: label,
        target: 'vpe',
        targetPort: label,
        label,
      });
      expect(edge.labelPosition).toBe('bend');
    }
  });

  it('does not mark ordinary unlabeled edges as bend labels', () => {
    const edge = toNormalReactFlowEdge({
      id: 'ordinary', source: 'a', sourcePort: 'out', target: 'b', targetPort: 'in',
    });
    expect(edge.labelPosition).toBeUndefined();
  });

  it('places the base left of a left-handle target so the arrow points right', () => {
    expect(getNormalDataArrowPoints(100, 50, 'left')).toBe('100,50 87,44 87,56');
  });

  it('orients the arrow from the routed final segment', () => {
    expect(getNormalDataArrowPointsFromSegment(100, 50, 80, 50)).toBe('100,50 87,44 87,56');
    expect(getNormalDataArrowPointsFromSegment(100, 50, 100, 30)).toBe('100,50 94,37 106,37');
  });
});
