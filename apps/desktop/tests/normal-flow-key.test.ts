import { describe, expect, it } from 'vitest';

import { normalFlowTopologyKey } from '../src/normal-flow-key.js';

describe('normalFlowTopologyKey', () => {
  it('changes when expansion state changes even if node ids are unchanged', () => {
    const edges = [{ id: 'e', source: 'vbe', target: 'vpe', label: 'Full YUV' }];
    const collapsed = [{ id: 'vbe', position: { x: 0, y: 0 }, data: { expanded: false } }];
    const expanded = [{ id: 'vbe', position: { x: 0, y: 0 }, data: { expanded: true } }];

    expect(normalFlowTopologyKey(collapsed, edges)).not.toBe(normalFlowTopologyKey(expanded, edges));
  });

  it('changes when a projected edge endpoint or label changes', () => {
    const nodes = [{ id: 'vbe', position: { x: 0, y: 0 }, data: { expanded: false } }];
    const boundary = [{ id: 'e', source: 'vbe', target: 'vpe', label: 'Full YUV / 1/4 YUV / 1/16 YUV' }];
    const internal = [{ id: 'e', source: 'rgb2yuv', target: 'vpe', label: 'Full YUV' }];

    expect(normalFlowTopologyKey(nodes, boundary)).not.toBe(normalFlowTopologyKey(nodes, internal));
  });

  it('changes when an edge moves to a different port handle', () => {
    const nodes = [{ id: 'pyrd', position: { x: 0, y: 0 }, data: { expanded: false } }];
    const full = [{ id: 'e', source: 'pyrd', sourceHandle: 'full', target: 'vpe', targetHandle: 'in' }];
    const quarter = [{ id: 'e', source: 'pyrd', sourceHandle: 'quarter', target: 'vpe', targetHandle: 'in' }];

    expect(normalFlowTopologyKey(nodes, full)).not.toBe(normalFlowTopologyKey(nodes, quarter));
  });
});
