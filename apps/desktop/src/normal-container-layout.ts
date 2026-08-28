import dagre from '@dagrejs/dagre';

import type { VisibleNormalEdge, VisibleNormalNode } from '../../../web/src/normal-flow.js';

const NODE_WIDTH = 154;
const NODE_HEIGHT = 72;
const PORT_SPACING = 18;
const FRAME_PADDING_X = 24;
const FRAME_HEADER_HEIGHT = 42;
const FRAME_PADDING_BOTTOM = 24;

export interface NormalLayoutConfig {
  readonly rankdir: 'LR' | 'TB';
  readonly ranksep: number;
  readonly nodesep: number;
  readonly edgesep: number;
  readonly marginx: number;
  readonly marginy: number;
  readonly ranker: 'network-simplex';
}

export function normalLayoutConfig(parentId: string | null): NormalLayoutConfig {
  if (parentId === null) return { rankdir: 'TB', ranksep: 96, nodesep: 48, edgesep: 28, marginx: 20, marginy: 20, ranker: 'network-simplex' };
  if (parentId === 'vpe') return { rankdir: 'TB', ranksep: 72, nodesep: 40, edgesep: 28, marginx: 12, marginy: 12, ranker: 'network-simplex' };
  return { rankdir: 'LR', ranksep: 76, nodesep: 36, edgesep: 24, marginx: 12, marginy: 12, ranker: 'network-simplex' };
}

export interface NormalContainerLayoutNode extends VisibleNormalNode {
  readonly frame: boolean;
  readonly parentId: string | null;
  readonly position: { readonly x: number; readonly y: number };
  readonly width: number;
  readonly height: number;
}

export interface NormalContainerLayout {
  readonly nodes: readonly NormalContainerLayoutNode[];
  readonly edges: readonly VisibleNormalEdge[];
}

interface SizedNode extends VisibleNormalNode {
  readonly frame: boolean;
  readonly width: number;
  readonly height: number;
  readonly children: readonly PositionedNode[];
}

interface PositionedNode extends SizedNode {
  readonly position: { readonly x: number; readonly y: number };
}

export function layoutNormalContainers(
  nodes: readonly VisibleNormalNode[],
  edges: readonly VisibleNormalEdge[],
): NormalContainerLayout {
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const childrenByParent = new Map<string | null, string[]>();
  for (const node of nodes) {
    const parentId = node.parentId !== null && nodeById.has(node.parentId) ? node.parentId : null;
    const children = childrenByParent.get(parentId) ?? [];
    children.push(node.id);
    childrenByParent.set(parentId, children);
  }

  const sizedById = new Map<string, SizedNode>();
  const sizeNode = (id: string): SizedNode => {
    const existing = sizedById.get(id);
    if (existing !== undefined) return existing;
    const node = nodeById.get(id);
    if (node === undefined) throw new Error(`Unknown Normal node ${id}`);
    const childIds = childrenByParent.get(id) ?? [];
    const frame = node.kind === 'group' && node.expanded && childIds.length > 0;
    if (!frame) {
      const portCount = Math.max(node.inputs.length, node.outputs.length);
      const height = Math.max(NODE_HEIGHT, (portCount + 1) * PORT_SPACING);
      const sized = { ...node, frame: false, width: NODE_WIDTH, height, children: [] };
      sizedById.set(id, sized);
      return sized;
    }
    const children = childIds.map(sizeNode);
    const positioned = layoutSiblings(id, children, edges, nodeById);
    const width = Math.max(NODE_WIDTH, ...positioned.map((child) => child.position.x + child.width + FRAME_PADDING_X));
    const height = Math.max(NODE_HEIGHT, ...positioned.map((child) => child.position.y + child.height + FRAME_PADDING_BOTTOM));
    const sized = { ...node, frame: true, width, height, children: positioned };
    sizedById.set(id, sized);
    return sized;
  };

  const topLevel = (childrenByParent.get(null) ?? []).map(sizeNode);
  const positionedTopLevel = layoutSiblings(null, topLevel, edges, nodeById);
  const flattened: NormalContainerLayoutNode[] = [];
  const append = (node: PositionedNode, parentId: string | null): void => {
    flattened.push({
      id: node.id,
      label: node.label,
      kind: node.kind,
      mode: node.mode,
      executionNodeId: node.executionNodeId,
      moduleId: node.moduleId,
      iqOverrideId: node.iqOverrideId,
      inputs: node.inputs,
      outputs: node.outputs,
      expanded: node.expanded,
      frame: node.frame,
      parentId,
      position: node.position,
      width: node.width,
      height: node.height,
    });
    for (const child of node.children) append(child, node.id);
  };
  for (const node of positionedTopLevel) append(node, null);
  return { nodes: flattened, edges };
}

function layoutSiblings(
  parentId: string | null,
  nodes: readonly SizedNode[],
  edges: readonly VisibleNormalEdge[],
  nodeById: ReadonlyMap<string, VisibleNormalNode>,
): PositionedNode[] {
  if (nodes.length === 0) return [];
  if (parentId === 'vbe') return layoutVbeRows(nodes, edges);
  const graph = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  graph.setGraph(normalLayoutConfig(parentId));
  for (const node of nodes) graph.setNode(node.id, { width: node.width, height: node.height });
  const childIds = new Set(nodes.map((node) => node.id));
  const edgeConfigByKey = new Map<string, { source: string; target: string; weight: number; minlen: number }>();
  for (const edge of edges) {
    const source = directChild(edge.source, parentId, nodeById);
    const target = directChild(edge.target, parentId, nodeById);
    if (source === null || target === null || source === target || !childIds.has(source) || !childIds.has(target)) continue;
    const key = `${source}->${target}`;
    const weight = 2;
    const minlen = 1;
    const current = edgeConfigByKey.get(key);
    if (current === undefined || current.weight < weight) edgeConfigByKey.set(key, { source, target, weight, minlen });
  }
  for (const edge of edgeConfigByKey.values()) graph.setEdge(edge.source, edge.target, { weight: edge.weight, minlen: edge.minlen });
  dagre.layout(graph);
  const raw = nodes.map((node) => {
    const point = graph.node(node.id) as { x: number; y: number };
    return { node, x: point.x - node.width / 2, y: point.y - node.height / 2 };
  });
  const minX = Math.min(...raw.map((item) => item.x));
  const minY = Math.min(...raw.map((item) => item.y));
  const offsetX = parentId === null ? 0 : FRAME_PADDING_X;
  const offsetY = parentId === null ? 0 : FRAME_HEADER_HEIGHT;
  const config = normalLayoutConfig(parentId);
  const maxWidth = Math.max(...nodes.map((node) => node.width));
  return raw.map(({ node, x, y }) => ({
    ...node,
    position: {
      x: config.rankdir === 'TB' ? (maxWidth - node.width) / 2 + offsetX : x - minX + offsetX,
      y: y - minY + offsetY,
    },
  }));
}

function layoutVbeRows(nodes: readonly SizedNode[], edges: readonly VisibleNormalEdge[]): PositionedNode[] {
  const secondRowById: Record<string, true> = { pfr: true, color_correction: true, gamma: true, three_d_lut: true, rgb2yuv: true, pyrd: true };
  const firstRow = nodes.filter((node) => secondRowById[node.id] !== true);
  const secondRow = nodes.filter((node) => secondRowById[node.id] === true);
  const first = layoutVbeRow(firstRow, edges, FRAME_HEADER_HEIGHT);
  const firstHeight = Math.max(...first.map((node) => node.height), NODE_HEIGHT);
  const second = layoutVbeRow(secondRow, edges, FRAME_HEADER_HEIGHT + firstHeight + 56);
  return [...first, ...second];
}

function layoutVbeRow(
  nodes: readonly SizedNode[],
  edges: readonly VisibleNormalEdge[],
  y: number,
): PositionedNode[] {
  const graph = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  graph.setGraph(normalLayoutConfig('vbe'));
  const ids = new Set(nodes.map((node) => node.id));
  for (const node of nodes) graph.setNode(node.id, { width: node.width, height: node.height });
  for (const edge of edges) {
    if (ids.has(edge.source) && ids.has(edge.target)) graph.setEdge(edge.source, edge.target, { weight: 2, minlen: 1 });
  }
  dagre.layout(graph);
  const positioned = nodes.map((node) => {
    const point = graph.node(node.id) as { x: number };
    return { node, x: point.x - node.width / 2 };
  });
  const minX = Math.min(...positioned.map((item) => item.x));
  return positioned.map(({ node, x }) => ({
    ...node,
    position: { x: x - minX + FRAME_PADDING_X, y },
  }));
}

function directChild(
  id: string,
  parentId: string | null,
  nodeById: ReadonlyMap<string, VisibleNormalNode>,
): string | null {
  let current = nodeById.get(id);
  if (current === undefined) return null;
  while (true) {
    const currentParent = current.parentId !== null && nodeById.has(current.parentId) ? current.parentId : null;
    if (currentParent === parentId) return current.id;
    if (currentParent === null) return null;
    const parent = nodeById.get(currentParent);
    if (parent === undefined) return null;
    current = parent;
  }
}
