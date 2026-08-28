import { describe, expect, it } from 'vitest';

import { normalManifest } from '../src/generated/normal_manifest.generated.js';
import { buildDagFlow } from '../src/dag-flow.js';

describe('buildDagFlow', () => {
  it('preserves manifest nodes and directed edges', () => {
    const flow = buildDagFlow(normalManifest);

    expect(flow.nodes.map((node) => node.id)).toEqual([
      'raw_source', 'blc', 'sbpc_horizontal', 'dbpc', 'sbpc', 'tintless', 'lsc',
      'hr', 'drc', 'cac', 'raw_nr', 'wbc', 'dem', 'pfr', 'color_correction', 'gamma',
      'three_d_lut', 'rgb2yuv',
    ]);
    expect(flow.edges.map((edge) => [edge.source, edge.target])).toEqual([
      ['raw_source', 'blc'], ['blc', 'sbpc_horizontal'],
      ['sbpc_horizontal', 'dbpc'], ['dbpc', 'sbpc'], ['sbpc', 'tintless'],
      ['tintless', 'lsc'], ['lsc', 'hr'], ['hr', 'drc'], ['drc', 'cac'], ['cac', 'raw_nr'],
      ['raw_nr', 'wbc'], ['wbc', 'dem'], ['dem', 'pfr'], ['pfr', 'color_correction'],
      ['color_correction', 'gamma'], ['gamma', 'three_d_lut'], ['three_d_lut', 'rgb2yuv'],
    ]);
  });
});
