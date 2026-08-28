import type { Edge, Node } from '@xyflow/react';

export function normalFlowTopologyKey(nodes: readonly Node[], edges: readonly Edge[]): string {
  const nodeSignature = nodes.map((node) => `${node.id}:${String(node.data.expanded ?? false)}`).join('|');
  const edgeSignature = edges
    .map((edge) => `${edge.source}:${String(edge.sourceHandle ?? '')}>${edge.target}:${String(edge.targetHandle ?? '')}:${String(edge.label ?? '')}`)
    .join('|');
  return `${nodeSignature}::${edgeSignature}`;
}
