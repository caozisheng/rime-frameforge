import type { normalManifest } from './generated/normal_manifest.generated.js';

type NormalManifest = typeof normalManifest;
export interface DagFlowNodeData {
  readonly [key: string]: unknown;
  readonly displayName: string;
  readonly index: number;
}
export interface DagFlowNode { readonly id: string; readonly position: { readonly x: number; readonly y: number }; readonly data: DagFlowNodeData; readonly type: 'dag' }
export interface DagFlowEdge { readonly id: string; readonly source: string; readonly target: string }
export interface DagFlow { readonly nodes: readonly DagFlowNode[]; readonly edges: readonly DagFlowEdge[] }

export function buildDagFlow(manifest: NormalManifest): DagFlow {
  return {
    nodes: manifest.nodes.map((node, index) => ({ id: node.id, position: { x: index * 220, y: 0 }, data: { displayName: node.display_name, index }, type: 'dag' })),
    edges: manifest.edges.map((edge) => ({ id: edge.id, source: edge.from.node_id, target: edge.to.node_id })),
  };
}
