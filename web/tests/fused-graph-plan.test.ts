import { describe, expect, it } from 'vitest';

import { buildFusedGraphPlan } from '../src/gpu/fused-graph-plan.js';

describe('fused Normal Graph plan', () => {
  it('keeps enabled nodes in pull order and removes bypass operators', () => {
    const plan = buildFusedGraphPlan();

    expect(plan.nodes.map((node) => node.id)).toEqual([
      'blc',
      'wbc',
      'dem',
      'color_correction',
      'gamma',
      'rgb2yuv',
    ]);
    expect(plan.bypassedNodeIds).toEqual([
      'sbpc_horizontal',
      'dbpc',
      'sbpc',
      'tintless',
      'lsc',
      'hr',
      'drc',
      'cac',
      'raw_nr',
      'pfr',
      'three_d_lut',
    ]);
  });

  it('identifies the RAW source and preview output boundary', () => {
    const plan = buildFusedGraphPlan();

    expect(plan.sourceNodeId).toBe('raw_source');
    expect(plan.previewNodeId).toBe('rgb2yuv');
    expect(plan.nodes.at(-1)?.output.format).toBe('rgba32_float');
  });
});
