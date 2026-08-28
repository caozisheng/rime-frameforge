import type { DngFrameDescriptor } from '../runtime/worker-bridge.js';
import { DngMetadataTree } from './DngMetadataTree.js';
import { InspectorTree, type InspectorTreeGroup } from './InspectorTree.js';
import type { RuntimeEnvelope } from '../../../../web/src/contracts.js';
import { normalManifest } from '../../../../web/src/generated/normal_manifest.generated.js';
import { normalGraphPresentation } from '../../../../web/src/generated/normal_graph.generated.js';

interface NodeInspectorProps {
  readonly nodeId: string;
  readonly envelope: RuntimeEnvelope;
  readonly dngFrame: DngFrameDescriptor | null;
  readonly frameCount: number;
  readonly activeMethod: string;
  readonly parameterValues: Readonly<Record<string, string | number>>;
  readonly onMethodChange: (nodeId: string, method: string) => void;
  readonly onParameterChange: (nodeId: string, parameter: string, value: number) => void;
}

export function NodeInspector({ nodeId, envelope, dngFrame, frameCount, activeMethod, parameterValues, onMethodChange, onParameterChange }: NodeInspectorProps) {
  const treeNode = normalGraphPresentation.nodes.find((node) => node.id === nodeId || node.execution_node_id === nodeId)
    ?? normalGraphPresentation.nodes[0];
  const executionNode = treeNode.execution_node_id === null
    ? undefined
    : normalManifest.nodes.find((node) => node.id === treeNode.execution_node_id);
  const output = executionNode?.outputs[0];
  const input = executionNode?.inputs[0];
  const backend = executionNode?.shader_entry === null ? 'asset' : executionNode ? 'wgsl' : 'none';
  const selectedMethod = executionNode?.methods.find((method) => method.method === activeMethod)
    ?? executionNode?.methods.find((method) => method.method === executionNode.default_method);
  const canConfigure = envelope.lifecycleState === 'stop' || envelope.lifecycleState === 'completed';
  const methodControl = executionNode === undefined || executionNode.methods.length === 0
    ? undefined
    : <select aria-label={`${executionNode.id} method`} disabled={!canConfigure} value={selectedMethod?.method ?? executionNode.default_method} onChange={(event) => onMethodChange(executionNode.id, event.target.value)}>{executionNode.methods.map((method) => <option key={method.method} value={method.method}>{method.method} · {method.shader_entry.replace(/^demosaic_|_main$/g, '')}</option>)}</select>;
  const groups: readonly InspectorTreeGroup[] = [
    {
      id: 'general', label: 'General', defaultExpanded: true, children: [
        { id: 'general.mode', label: 'Mode', value: treeNode.mode },
        { id: 'general.backend', label: 'Backend', value: backend },
        { id: 'general.input', label: 'Input', value: input?.domain ?? 'unavailable' },
        { id: 'general.output', label: 'Output', value: output?.domain ?? 'unavailable' },
        { id: 'general.method', label: 'Method', value: selectedMethod?.method ?? '—', control: methodControl },
        { id: 'general.shader', label: 'Shader', value: selectedMethod?.shader_entry ?? executionNode?.shader_entry ?? '—' },
        { id: 'general.frame', label: 'Frame', value: `${envelope.frameIndex ?? '—'} / ${envelope.framePhase ?? 'idle'}` },
        { id: 'general.runRevision', label: 'Run revision', value: String(envelope.runRevision) },
        { id: 'general.methodRevision', label: 'Method revision', value: String(envelope.methodRevision) },
        { id: 'general.gpuGeneration', label: 'GPU generation', value: String(envelope.gpuGeneration) },
        ...(treeNode.reason === null ? [] : [{ id: 'general.reason', label: 'Reason', value: treeNode.reason ?? '—' }]),
      ],
    },
    {
      id: 'parameters', label: 'Parameters', defaultExpanded: true,
      children: selectedMethod === undefined
        ? [{ id: 'parameters.empty', label: 'Value', value: 'No parameters' }]
        : selectedMethod.parameters.map((parameter) => ({
          id: `parameters.${parameter}`,
          label: parameter,
          value: String(parameterValues[parameter] ?? dngFrame?.cfa ?? '—'),
          control: parameter === 'cfa_pattern' || executionNode?.id !== 'dem'
            ? <output>{parameterValues[parameter] ?? dngFrame?.cfa ?? '—'}</output>
            : <input aria-label={parameter} disabled={!canConfigure} type="number" step="0.1" value={parameterValues[parameter] ?? ''} onChange={(event) => onParameterChange(executionNode.id, parameter, Number(event.target.value))} />,
        })),
    },
  ];

  return (
    <aside className="panel inspector-panel" aria-labelledby="inspector-heading">
      <div className="panel-heading compact">
        <div>
          <span className="section-label">Node inspector</span>
          <h2 id="inspector-heading">{treeNode.label}</h2>
        </div>
        <span className={`tree-mode-badge mode-${treeNode.mode}`}>{treeNode.mode}</span>
      </div>
      {treeNode.id === 'raw_source' ? (
        dngFrame === null
          ? <div className="dng-empty-state"><strong>No DNG frame loaded</strong><span>Load a DNG to inspect the active frame metadata.</span></div>
          : <DngMetadataTree descriptor={dngFrame} lifecycleState={envelope.lifecycleState} frameIndex={envelope.frameIndex} frameCount={frameCount} />
      ) : <>
        <InspectorTree key={treeNode.id} ariaLabel={`${treeNode.label} inspector`} groups={groups} storageKey={`rime:node-inspector:${treeNode.id}`} />
        <div className="inspector-note">{treeNode.execution_node_id === null ? 'Architecture group; no executable runtime node.' : 'Mapped to the active Normal Graph runtime operator.'}</div>
      </>}
    </aside>
  );
}
