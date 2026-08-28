import { describe, expect, it } from 'vitest';

import { normalGraphPresentation } from '../../../web/src/generated/normal_graph.generated.js';
import { projectNormalGraph } from '../../../web/src/normal-flow.js';
import { layoutNormalContainers, normalLayoutConfig } from '../src/normal-container-layout.js';

const expanded = new Set(['normal', 'vfe', 'vbe', 'vpe', 'vpe_16_pass', 'vpe_4_pass', 'vpe_full_pass']);

describe('layoutNormalContainers', () => {
  it('nests operators inside VFE VBE and VPE frames', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('vfe')?.frame).toBe(true);
    expect(byId.get('blc')?.parentId).toBe('vfe');
    expect(byId.get('vfe')?.width).toBeGreaterThan(154);
    expect(byId.get('vbe')?.frame).toBe(true);
    expect(byId.get('rgb2yuv')?.parentId).toBe('vbe');
    expect(byId.get('vpe')?.frame).toBe(true);
  });

  it('nests VPE operators inside pass frames', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('vpe_16_pass')?.parentId).toBe('vpe');
    expect(byId.get('vpe_16_pass')?.frame).toBe(true);
    expect(byId.get('vpe_16_pyrc')?.parentId).toBe('vpe_16_pass');
    expect(byId.get('vpe_full_sharpen')?.parentId).toBe('vpe_full_pass');
  });

  it('keeps only the VFE image chain in the layout graph', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const ids = new Set(projected.nodes.map((node) => node.id));

    for (const id of ['ae_awb_st', 'afst', 'spc', 'lrc', 'pdst']) expect(ids.has(id)).toBe(false);
    expect(projected.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: 'tintless', target: 'lsc' }),
    ]));
  });

  it('preserves shared MCTF module and IQ override bindings', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);

    for (const prefix of ['vpe_16', 'vpe_4', 'vpe_full']) {
      expect(layout.nodes.find((node) => node.id === `${prefix}_mctf_1`)).toEqual(expect.objectContaining({
        label: 'MCTF', moduleId: 'mctf', iqOverrideId: 'mctf_1',
      }));
      expect(layout.nodes.find((node) => node.id === `${prefix}_mctf_2`)).toEqual(expect.objectContaining({
        label: 'MCTF', moduleId: 'mctf', iqOverrideId: 'mctf_2',
      }));
    }
  });

  it('uses a compact node when VFE is collapsed', () => {
    const projected = projectNormalGraph(normalGraphPresentation, new Set(['normal', 'vbe']));
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const vfe = layout.nodes.find((node) => node.id === 'vfe');

    expect(vfe?.frame).toBe(false);
    expect(vfe?.width).toBe(154);
    expect(layout.nodes.some((node) => node.id === 'blc')).toBe(false);
  });

  it('grows a compact node to keep many handles separated', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const nodes = projected.nodes.map((node) => node.id === 'pyrd'
      ? { ...node, outputs: ['one', 'two', 'three', 'four', 'five'] }
      : node);
    const layout = layoutNormalContainers(nodes, projected.edges);

    expect(layout.nodes.find((node) => node.id === 'pyrd')?.height).toBe(108);
  });

  it('keeps RAW Source and FFmpeg Encoder as top-level endpoint nodes', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('raw_source')?.parentId).toBe(null);
    expect(byId.get('encoder')?.parentId).toBe(null);
  });

  it('stacks VPE pass frames vertically', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('vpe_4_pass')!.position.y).toBeGreaterThan(byId.get('vpe_16_pass')!.position.y);
    expect(byId.get('vpe_full_pass')!.position.y).toBeGreaterThan(byId.get('vpe_4_pass')!.position.y);
  });

  it('uses generous Dagre separation for top-level and nested routing', () => {
    expect(normalLayoutConfig(null).ranksep).toBe(96);
    expect(normalLayoutConfig('vfe').rankdir).toBe('LR');
    expect(normalLayoutConfig('vpe').rankdir).toBe('TB');
  });

  it('places PFR on the second VBE row before CCM', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('pfr')!.position.y).toBe(byId.get('color_correction')!.position.y);
    expect(byId.get('pfr')!.position.x).toBeLessThan(byId.get('color_correction')!.position.x);
    expect(byId.get('pfr')!.position.y).toBeGreaterThan(byId.get('wbc')!.position.y);
  });

  it('keeps all VBE to VPE scale labels at routed bends', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const scaleEdges = projected.edges.filter((edge) => edge.source === 'pyrd');

    expect(scaleEdges).toHaveLength(3);
    expect(scaleEdges.every((edge) => edge.labelPosition === 'bend')).toBe(true);
  });

  it('does not increase stage separation for labeled edges', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const labeled = layoutNormalContainers(projected.nodes, projected.edges);
    const unlabeled = layoutNormalContainers(
      projected.nodes,
      projected.edges.map(({ label: _label, ...edge }) => edge),
    );
    const stageY = (layout: typeof labeled, id: string): number => layout.nodes.find((node) => node.id === id)!.position.y;

    expect(stageY(labeled, 'vpe') - stageY(labeled, 'vbe')).toBe(stageY(unlabeled, 'vpe') - stageY(unlabeled, 'vbe'));
  });

  it('wraps VBE after DEM/PFR into a second horizontal row', () => {
    const projected = projectNormalGraph(normalGraphPresentation, expanded);
    const layout = layoutNormalContainers(projected.nodes, projected.edges);
    const byId = new Map(layout.nodes.map((node) => [node.id, node]));

    expect(byId.get('color_correction')!.position.y).toBeGreaterThan(byId.get('wbc')!.position.y);
    expect(byId.get('rgb2yuv')!.position.y).toBeGreaterThan(byId.get('dem')!.position.y);
  });
});
