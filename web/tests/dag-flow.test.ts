import { describe, expect, it } from 'vitest';

import { normalManifest } from '../src/generated/normal_manifest.generated.js';
import { buildDagFlow } from '../src/dag-flow.js';

describe('buildDagFlow', () => {
  it('preserves manifest nodes and directed edges', () => {
    const flow = buildDagFlow(normalManifest);

    expect(flow.nodes.map((node) => node.id)).toEqual([
      'raw_source', 'blc', 'sbpc_horizontal', 'dbpc', 'sbpc', 'raw_nr', 'tintless', 'lsc',
      'wbc', 'drc', 'dem', 'color_reproduce', 'rgb2yuv', 'lcst',
    ]);
    expect(flow.edges.map((edge) => [edge.source, edge.target])).toEqual([
      ['raw_source', 'blc'], ['blc', 'sbpc_horizontal'],
      ['sbpc_horizontal', 'dbpc'], ['dbpc', 'sbpc'], ['sbpc', 'raw_nr'],
      ['raw_nr', 'tintless'], ['tintless', 'lsc'], ['lsc', 'wbc'], ['wbc', 'drc'], ['drc', 'dem'],
      ['dem', 'color_reproduce'], ['color_reproduce', 'rgb2yuv'],
      ['sbpc', 'lcst'], ['lcst', 'tintless'], ['lcst', 'drc'],
    ]);
  });
});
