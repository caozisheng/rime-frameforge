import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Background, Controls, Handle, ReactFlow, type Edge, type Node, type NodeProps, type ReactFlowInstance } from '@xyflow/react';
import { SmartEdgeProvider, createSmartEdge } from '@tisoap/react-flow-smart-edge';
import '@xyflow/react/dist/style.css';

import type { RuntimeEnvelope } from '../../../../web/src/contracts.js';
import { normalGraphPresentation } from '../../../../web/src/generated/normal_graph.generated.js';
import { projectNormalGraph, type VisibleNormalEdge, type VisibleNormalNode } from '../../../../web/src/normal-flow.js';
import { NormalDataEdge } from './NormalDataEdge.js';
import { layoutNormalContainers, type NormalContainerLayoutNode } from '../normal-container-layout.js';
import { getNormalPortHandles, normalHandlePositions, toNormalReactFlowEdge } from '../normal-edge.js';
import { normalFlowTopologyKey } from '../normal-flow-key.js';

interface NormalGraphCanvasProps {
  readonly envelope: RuntimeEnvelope;
  readonly onSelect: (nodeId: string) => void;
  readonly selectedNode: string;
  readonly fitRequest: number;
  readonly headingActions: ReactNode;
}

type NormalNodeData = VisibleNormalNode & {
  readonly [key: string]: unknown;
  readonly frame: boolean;
  readonly onToggle: (id: string) => void;
};
type NormalFlowNode = Node<NormalNodeData, 'normal' | 'normalFrame'>;

const EXPANSION_KEY = 'rime:normal-graph:expanded:v3';
const SmartSmoothEdge = createSmartEdge('smoothstep', { gridRatio: 6, nodePadding: 12 });
const nodeTypes = { normal: NormalNodeComponent, normalFrame: NormalFrameComponent };
const edgeTypes = { normalData: NormalDataEdge, smartSmooth: SmartSmoothEdge };

function initialExpanded(): Set<string> {
  const defaults = normalGraphPresentation.nodes.filter((node) => node.default_expanded).map((node) => node.id);
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(EXPANSION_KEY) ?? 'null');
    return new Set(Array.isArray(stored) && stored.every((id) => typeof id === 'string') ? stored : defaults);
  } catch {
    return new Set(defaults);
  }
}

export function NormalGraphCanvas({ envelope, onSelect, selectedNode, fitRequest, headingActions }: NormalGraphCanvasProps) {
  const instanceRef = useRef<ReactFlowInstance<NormalFlowNode, Edge> | null>(null);
  const [expanded, setExpanded] = useState(initialExpanded);
  const projected = useMemo(() => projectNormalGraph(normalGraphPresentation, expanded), [expanded]);
  const toggle = (id: string): void => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      localStorage.setItem(EXPANSION_KEY, JSON.stringify([...next]));
      return next;
    });
  };
  const flow = useMemo(() => buildFlow(projected.nodes, projected.edges, toggle, selectedNode), [projected, selectedNode]);
  const topologyKey = useMemo(() => normalFlowTopologyKey(flow.nodes, flow.edges), [flow]);

  useEffect(() => {
    if (fitRequest > 0) void instanceRef.current?.fitView({ padding: 0.08, duration: 120 });
  }, [fitRequest]);

  return (
    <section className="panel graph-panel" aria-labelledby="graph-heading">
      <div className="panel-heading compact">
        <div><span className="eyebrow">Executable ISP topology</span><h2 id="graph-heading">Normal Graph</h2></div>
        <div className="graph-heading-actions">
          <span className={`state-chip state-${envelope.lifecycleState}`}>{envelope.lifecycleState}</span>
          {headingActions}
        </div>
      </div>
      <div className="react-flow-canvas" aria-label="Normal Graph VFE VBE VPE architecture">
        <SmartEdgeProvider nodes={flow.nodes} options={{ gridRatio: 6, nodePadding: 12, routeOnlyWhenBlocked: false }}>
          <ReactFlow
            key={topologyKey}
            nodes={flow.nodes}
            edges={flow.edges}
            edgeTypes={edgeTypes}
            nodeTypes={nodeTypes}
            fitView
            fitViewOptions={{ padding: 0.08, minZoom: 0.08, maxZoom: 1 }}
            nodesDraggable={false}
            nodesConnectable={false}
            onInit={(instance) => { instanceRef.current = instance; }}
            onNodeClick={(_, node) => onSelect(node.data.executionNodeId ?? node.id)}
            proOptions={{ hideAttribution: true }}
          >
            <Background color="#d8dde3" gap={16} size={1} />
            <Controls showInteractive={false} />
          </ReactFlow>
        </SmartEdgeProvider>
      </div>
    </section>
  );
}

function NormalNodeComponent({ data, selected }: NodeProps<NormalFlowNode>) {
  return (
    <>
      {getNormalPortHandles(data.inputs).map((handle) => (
        <Handle key={`target:${handle.id}`} id={handle.id} type="target" position={normalHandlePositions.target} style={{ top: handle.top }} className="dag-handle" />
      ))}
      <div className={`graph-node normal-node mode-${data.mode} ${selected ? 'is-selected' : ''}`}>
        <div className="normal-node-title">
          {data.kind === 'group' && <button className="normal-group-toggle" type="button" onClick={(event) => { event.stopPropagation(); data.onToggle(data.id); }} aria-label={`${data.expanded ? 'Collapse' : 'Expand'} ${data.label}`}>{data.expanded ? '▾' : '▸'}</button>}
          <strong>{data.label}</strong>
        </div>
        <small>{data.kind === 'group' ? 'group' : data.kind === 'endpoint' ? 'external I/O' : data.mode}</small>
      </div>
      {getNormalPortHandles(data.outputs).map((handle) => (
        <Handle key={`source:${handle.id}`} id={handle.id} type="source" position={normalHandlePositions.source} style={{ top: handle.top }} className="dag-handle" />
      ))}
    </>
  );
}

function NormalFrameComponent({ data, selected }: NodeProps<NormalFlowNode>) {
  return (
    <>
      {getNormalPortHandles(data.inputs).map((handle) => (
        <Handle key={`target:${handle.id}`} id={handle.id} type="target" position={normalHandlePositions.target} style={{ top: handle.top }} className="dag-handle normal-frame-handle" />
      ))}
      <div className={`normal-group-frame mode-${data.mode} ${selected ? 'is-selected' : ''}`}>
        <div className="normal-group-header">
          <button className="normal-group-toggle" type="button" onClick={(event) => { event.stopPropagation(); data.onToggle(data.id); }} aria-label={`${data.expanded ? 'Collapse' : 'Expand'} ${data.label}`}>{data.expanded ? '▾' : '▸'}</button>
          <strong>{data.label}</strong>
          <span>{data.mode}</span>
        </div>
      </div>
      {getNormalPortHandles(data.outputs).map((handle) => (
        <Handle key={`source:${handle.id}`} id={handle.id} type="source" position={normalHandlePositions.source} style={{ top: handle.top }} className="dag-handle normal-frame-handle" />
      ))}
    </>
  );
}

function buildFlow(
  nodes: readonly VisibleNormalNode[],
  edges: readonly VisibleNormalEdge[],
  onToggle: (id: string) => void,
  selectedNode: string,
): { nodes: NormalFlowNode[]; edges: Edge[] } {
  const layout = layoutNormalContainers(nodes, edges);
  return {
    nodes: layout.nodes.map((node) => toFlowNode(node, onToggle, selectedNode)),
    edges: layout.edges.map(toNormalReactFlowEdge),
  };
}

function toFlowNode(node: NormalContainerLayoutNode, onToggle: (id: string) => void, selectedNode: string): NormalFlowNode {
  return {
    id: node.id,
    type: node.frame ? 'normalFrame' : 'normal',
    position: node.position,
    data: { ...node, onToggle },
    selected: node.id === selectedNode || node.executionNodeId === selectedNode,
    measured: { width: node.width, height: node.height },
    ...(node.parentId === null ? {} : { parentId: node.parentId, extent: 'parent' as const }),
    style: { width: node.width, height: node.height },
  };
}
