import { describe, expect, it } from 'vitest';

import { normalGraphPresentation } from '../src/generated/normal_graph.generated.js';
import { projectNormalGraph } from '../src/normal-flow.js';

const expanded = new Set(['normal', 'vfe', 'vbe', 'vpe', 'vpe_16_pass', 'vpe_4_pass', 'vpe_full_pass']);

describe('projectNormalGraph', () => {
  it('shows the canonical architecture without non-simulated VFE statistics', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);
    for (const id of ['ae_awb_st', 'afst', 'spc', 'lrc', 'pdst']) {
      expect(graph.nodes.some((node) => node.id === id)).toBe(false);
    }
    expect(graph.nodes.some((node) => node.id === 'pyrd')).toBe(true);
    expect(graph.nodes.some((node) => node.id === 'yuv_full')).toBe(false);
    expect(graph.nodes.some((node) => node.id === 'yuv_quarter')).toBe(false);
    expect(graph.nodes.some((node) => node.id === 'yuv_sixteenth')).toBe(false);
    expect(graph.nodes.some((node) => node.id === 'vpe_full_sharpen')).toBe(true);
  });

  it('uses HR and CAC in the VBE image chain', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);

    expect(graph.nodes.find((node) => node.id === 'hr')?.label).toBe('HR');
    expect(graph.nodes.find((node) => node.id === 'cac')?.label).toBe('CAC');
    expect(graph.nodes.some((node) => node.id === 'hlr' || node.id === 'raw_ds_cac')).toBe(false);
    expect(graph.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: 'lsc', target: 'hr' }),
      expect.objectContaining({ source: 'drc', target: 'cac' }),
      expect.objectContaining({ source: 'cac', target: 'raw_nr' }),
    ]));
  });

  it('uses separate SBPC Tintless and LSC nodes in order', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);

    for (const [id, label] of [['sbpc', 'SBPC'], ['tintless', 'TINTLESS'], ['lsc', 'LSC']] as const) {
      expect(graph.nodes.find((node) => node.id === id)?.label).toBe(label);
    }
    expect(graph.nodes.some((node) => node.id === 'sbpc_pdpc' || node.id === 'lsc_tintless')).toBe(false);
    expect(graph.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: 'dbpc', target: 'sbpc' }),
      expect.objectContaining({ source: 'sbpc', target: 'tintless' }),
      expect.objectContaining({ source: 'tintless', target: 'lsc' }),
      expect.objectContaining({ source: 'lsc', target: 'hr' }),
    ]));
  });

  it('projects shared MCTF module and position IQ overrides', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);

    for (const prefix of ['vpe_16', 'vpe_4', 'vpe_full']) {
      for (const position of ['mctf_1', 'mctf_2'] as const) {
        const node = graph.nodes.find((candidate) => candidate.id === `${prefix}_${position}`);
        expect(node?.label).toBe('MCTF');
        expect(node?.moduleId).toBe('mctf');
        expect(node?.iqOverrideId).toBe(position);
      }
    }
  });

  it('uses CE between LCE and the second MCTF instance', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);
    for (const prefix of ['vpe_16', 'vpe_4', 'vpe_full']) {
      expect(graph.nodes.find((node) => node.id === `${prefix}_ce`)?.label).toBe('CE');
      expect(graph.nodes.some((node) => node.id === `${prefix}_color`)).toBe(false);
      expect(graph.edges).toEqual(expect.arrayContaining([
        expect.objectContaining({ source: `${prefix}_lce`, target: `${prefix}_ce` }),
        expect.objectContaining({ source: `${prefix}_ce`, target: `${prefix}_mctf_2` }),
      ]));
    }
  });

  it('uses DEM then PFR before CCM', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);
    expect(graph.nodes.find((node) => node.id === 'dem')?.label).toBe('DEM');
    expect(graph.nodes.find((node) => node.id === 'pfr')?.label).toBe('PFR');
    expect(graph.nodes.some((node) => node.id === 'demosaic')).toBe(false);
    expect(graph.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: 'wbc', target: 'dem' }),
      expect.objectContaining({ source: 'dem', target: 'pfr' }),
      expect.objectContaining({ source: 'pfr', target: 'color_correction' }),
    ]));
  });

  it('preserves concrete ports for expanded multi-output nodes', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);
    const pyrd = graph.nodes.find((node) => node.id === 'pyrd');
    const edges = graph.edges.filter((edge) => edge.source === 'pyrd');

    expect(pyrd?.inputs).toEqual(['in']);
    expect(pyrd?.outputs).toEqual(['full', 'quarter', 'sixteenth']);
    expect(edges.map((edge) => [edge.sourcePort, edge.target, edge.targetPort])).toEqual([
      ['full', 'vpe_full_pyrc', 'in'],
      ['quarter', 'vpe_4_pyrc', 'in'],
      ['sixteenth', 'vpe_16_pyrc', 'in'],
    ]);
  });


  it('redirects boundary edges when VFE is collapsed', () => {
    const collapsed = new Set(expanded);
    collapsed.delete('vfe');
    const graph = projectNormalGraph(normalGraphPresentation, collapsed);

    expect(graph.nodes.some((node) => node.id === 'blc')).toBe(false);
    expect(graph.nodes.some((node) => node.id === 'vfe')).toBe(true);
    expect(graph.edges.some((edge) => edge.source === 'vfe' && edge.target === 'hr')).toBe(true);
    expect(graph.edges.every((edge) => edge.source !== edge.target)).toBe(true);
  });

  it('collapses VPE descendants into distinct labeled data edges', () => {
    const collapsed = new Set(expanded);
    collapsed.delete('vpe');
    const graph = projectNormalGraph(normalGraphPresentation, collapsed);
    const boundary = graph.edges.filter((edge) => edge.source === 'pyrd' && edge.target === 'vpe');

    expect(graph.nodes.some((node) => node.id === 'vpe_full_sharpen')).toBe(false);
    expect(boundary.map((edge) => edge.label)).toEqual(['Full YUV', '1/4 YUV', '1/16 YUV']);
    expect(graph.edges.some((edge) => edge.source === 'vpe' && edge.target === 'encoder')).toBe(true);
  });

  it('preserves nested group ownership for expanded containers', () => {
    const graph = projectNormalGraph(normalGraphPresentation, expanded);

    expect(graph.nodes.find((node) => node.id === 'blc')?.parentId).toBe('vfe');
    expect(graph.nodes.find((node) => node.id === 'vpe_16_pass')?.parentId).toBe('vpe');
    expect(graph.nodes.find((node) => node.id === 'vpe_16_pyrc')?.parentId).toBe('vpe_16_pass');
    expect(graph.nodes.find((node) => node.id === 'vpe_full_sharpen')?.parentId).toBe('vpe_full_pass');
  });

  it('keeps distinct VBE to VPE ports when both groups are collapsed', () => {
    const collapsed = new Set(expanded);
    collapsed.delete('vbe');
    collapsed.delete('vpe');
    const graph = projectNormalGraph(normalGraphPresentation, collapsed);
    const boundary = graph.edges.filter((edge) => edge.source === 'vbe' && edge.target === 'vpe');

    expect(boundary).toHaveLength(3);
    expect(boundary.map((edge) => [edge.sourcePort, edge.targetPort])).toEqual([
      ['pyrd:full', 'vpe_full_pyrc:in'],
      ['pyrd:quarter', 'vpe_4_pyrc:in'],
      ['pyrd:sixteenth', 'vpe_16_pyrc:in'],
    ]);
  });


  it('keeps distinct PYRD ports when VPE is collapsed', () => {
    const partial = new Set(expanded);
    partial.delete('vpe');
    const graph = projectNormalGraph(normalGraphPresentation, partial);
    const fromPyrd = graph.edges.filter((edge) => edge.source === 'pyrd' && edge.target === 'vpe');

    expect(fromPyrd).toHaveLength(3);
    expect(fromPyrd.map((edge) => [edge.sourcePort, edge.targetPort])).toEqual([
      ['full', 'vpe_full_pyrc:in'],
      ['quarter', 'vpe_4_pyrc:in'],
      ['sixteenth', 'vpe_16_pyrc:in'],
    ]);
  });


  it('projects every edge endpoint to a visible node when all stages are collapsed', () => {
    const graph = projectNormalGraph(normalGraphPresentation, new Set(['normal']));
    const ids = new Set(graph.nodes.map((node) => node.id));

    expect(graph.edges.every((edge) => ids.has(edge.source) && ids.has(edge.target))).toBe(true);
    expect(graph.edges.map((edge) => [edge.source, edge.target])).toEqual([
      ['raw_source', 'vfe'],
      ['vfe', 'vbe'],
      ['vpe', 'encoder'],
      ['vbe', 'vpe'],
      ['vbe', 'vpe'],
      ['vbe', 'vpe'],
    ]);
  });
});
