import { normalGraphPresentation } from '../generated/normal_graph.generated.js';
import { normalManifest } from '../generated/normal_manifest.generated.js';

export interface FusedGraphNode {
  readonly id: string;
  readonly input: (typeof normalManifest.nodes)[number]['inputs'][number];
  readonly output: (typeof normalManifest.nodes)[number]['outputs'][number];
}

export interface FusedGraphPlan {
  readonly sourceNodeId: string;
  readonly previewNodeId: string;
  readonly nodes: readonly FusedGraphNode[];
  readonly bypassedNodeIds: readonly string[];
}

export function buildFusedGraphPlan(): FusedGraphPlan {
  const sourceNodeId = 'raw_source';
  const preview = normalManifest.preview_outputs[0];
  if (preview === undefined) throw new Error('FUSED_GRAPH_INVALID: missing preview output');
  const presentationByExecutionId = new Map(
    normalGraphPresentation.nodes
      .filter((node) => node.execution_node_id !== null)
      .map((node) => [node.execution_node_id, node] as const),
  );
  const nodes: FusedGraphNode[] = [];
  const bypassedNodeIds: string[] = [];
  for (const node of normalManifest.nodes) {
    if (node.id === sourceNodeId) continue;
    const presentation = presentationByExecutionId.get(node.id);
    if (presentation === undefined) throw new Error(`FUSED_GRAPH_INVALID: ${node.id} has no presentation mode`);
    if (presentation.mode === 'bypass') {
      bypassedNodeIds.push(node.id);
      continue;
    }
    if (presentation.mode !== 'enabled') continue;
    const input = node.inputs[0];
    const output = node.outputs[0];
    if (input === undefined || output === undefined) {
      throw new Error(`FUSED_GRAPH_INVALID: ${node.id} must have one input and one output`);
    }
    nodes.push({ id: node.id, input, output });
  }
  if (nodes.at(-1)?.id !== preview.node_id) {
    throw new Error(`FUSED_GRAPH_INVALID: preview ${preview.node_id} is not the fused sink`);
  }
  return { sourceNodeId, previewNodeId: preview.node_id, nodes, bypassedNodeIds };
}
