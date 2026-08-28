import type { normalGraphPresentation } from './generated/normal_graph.generated.js';

type NormalPresentation = typeof normalGraphPresentation;
type NormalNode = NormalPresentation['nodes'][number];

export interface VisibleNormalNode {
  readonly id: string;
  readonly label: string;
  readonly kind: NormalNode['kind'];
  readonly mode: NormalNode['mode'];
  readonly executionNodeId: string | null;
  readonly moduleId: string | null;
  readonly iqOverrideId: string | null;
  readonly parentId: string | null;
  readonly inputs: readonly string[];
  readonly outputs: readonly string[];
  readonly expanded: boolean;
}
export interface VisibleNormalEdge {
  readonly id: string;
  readonly source: string;
  readonly target: string;
  readonly sourcePort: string;
  readonly targetPort: string;
  readonly label?: string;
  readonly labelPosition?: 'bend';
}

export interface VisibleNormalGraph {
  readonly nodes: readonly VisibleNormalNode[];
  readonly edges: readonly VisibleNormalEdge[];
}

export function projectNormalGraph(presentation: NormalPresentation, expanded: ReadonlySet<string>): VisibleNormalGraph {
  const nodeById = new Map<string, NormalNode>(presentation.nodes.map((node) => [node.id, node]));

  const collapsedAncestor = (id: string): NormalNode | null => {
    let current = nodeById.get(id);
    let outermost: NormalNode | null = null;
    while (current?.parent_id !== null && current?.parent_id !== undefined) {
      const parent = nodeById.get(current.parent_id);
      if (parent === undefined) break;
      if (parent.kind === 'group' && !expanded.has(parent.id)) outermost = parent;
      current = parent;
    }
    return outermost;
  };

  const endpoint = (id: string): string => collapsedAncestor(id)?.id ?? id;
  const nodes: Array<VisibleNormalNode & { inputs: string[]; outputs: string[] }> = presentation.nodes
    .filter((node) => node.id !== presentation.root_id && collapsedAncestor(node.id) === null)
    .map((node) => ({
      id: node.id,
      label: node.label,
      kind: node.kind,
      mode: node.mode,
      executionNodeId: node.execution_node_id,
      moduleId: node.module_id,
      iqOverrideId: node.iq_override_id,
      parentId: node.parent_id,
      expanded: node.kind === 'group' && expanded.has(node.id),
      inputs: [...node.inputs],
      outputs: [...node.outputs],
    }));
  const visibleNodeById = new Map(nodes.map((node) => [node.id, node]));
  const edges: VisibleNormalEdge[] = [];
  const seen = new Set<string>();

  for (const edge of presentation.edges) {
    const target = endpoint(edge.to);
    const source = endpoint(edge.from);
    if (source === target) continue;
    const sourcePort = source === edge.from ? edge.from_port : `${edge.from}:${edge.from_port}`;
    const targetPort = target === edge.to ? edge.to_port : `${edge.to}:${edge.to_port}`;
    const key = `${source}:${sourcePort}->${target}:${targetPort}`;
    if (seen.has(key)) continue;
    seen.add(key);

    const sourceNode = visibleNodeById.get(source);
    const targetNode = visibleNodeById.get(target);
    if (sourceNode !== undefined && !sourceNode.outputs.includes(sourcePort)) sourceNode.outputs.push(sourcePort);
    if (targetNode !== undefined && !targetNode.inputs.includes(targetPort)) targetNode.inputs.push(targetPort);
    edges.push({
      id: edge.id,
      source,
      sourcePort,
      target,
      targetPort,
      ...(edge.label == null ? {} : { label: edge.label, labelPosition: 'bend' as const }),
    });
  }

  return { nodes, edges };
}
