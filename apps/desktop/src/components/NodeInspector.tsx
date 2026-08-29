import type { ReactNode } from 'react';

import type { RuntimeEnvelope } from '../../../../web/src/contracts.js';
import { normalGraphPresentation } from '../../../../web/src/generated/normal_graph.generated.js';
import { normalManifest } from '../../../../web/src/generated/normal_manifest.generated.js';
import { normalGraphQuantization } from '../../../../web/src/generated/normal_quantization.generated.js';
import type { DngFrameDescriptor } from '../runtime/worker-bridge.js';
import { DngMetadataTree } from './DngMetadataTree.js';
import { InspectorTree, type InspectorTreeGroup, type InspectorTreeNode } from './InspectorTree.js';

export interface ModuleQuantizationPreference {
  readonly module_id: string;
  readonly output_enabled: boolean;
  readonly output_profile: string;
  readonly dither_enabled: boolean;
  readonly clip_type: 'truncate' | 'round' | 'dither';
}

export interface GraphQuantizationConfig {
  readonly graph_id: string;
  readonly enabled: boolean;
  readonly modules: readonly ModuleQuantizationPreference[];
}

interface NodeInspectorProps {
  readonly nodeId: string | null;
  readonly envelope: RuntimeEnvelope;
  readonly dngFrame: DngFrameDescriptor | null;
  readonly frameCount: number;
  readonly activeMethod: string;
  readonly parameterValues: Readonly<Record<string, string | number>>;
  readonly quantization?: GraphQuantizationConfig;
  readonly onMethodChange: (nodeId: string, method: string) => void;
  readonly onParameterChange: (nodeId: string, parameter: string, value: number) => void;
  readonly onGraphQuantizationChange?: (config: GraphQuantizationConfig) => void;
  readonly onModuleQuantizationChange?: (moduleId: string, preference: ModuleQuantizationPreference) => void;
}

type PresentationNode = (typeof normalGraphPresentation.nodes)[number];
const defaultQuantization: GraphQuantizationConfig = normalGraphQuantization;
const PROFILE_OPTIONS = ['u0.10', 'u0.12', 'u0.14'] as const;
const CLIP_OPTIONS = ['truncate', 'round', 'dither'] as const;

function moduleControls(
  node: PresentationNode,
  preference: ModuleQuantizationPreference,
  config: GraphQuantizationConfig,
  canConfigure: boolean,
  onChange: (preference: ModuleQuantizationPreference) => void,
): readonly InspectorTreeNode[] {
  const forcedOff = !config.enabled || node.mode === 'disabled' || node.mode === 'bypass';
  const controlsDisabled = !canConfigure || forcedOff;
  const effectiveOutput = !forcedOff && preference.output_enabled;
  const effectiveDither = effectiveOutput && preference.dither_enabled;
  const update = <K extends keyof ModuleQuantizationPreference>(key: K, value: ModuleQuantizationPreference[K]): void => {
    onChange({ ...preference, [key]: value });
  };
  const profileOptions = PROFILE_OPTIONS.includes(preference.output_profile as typeof PROFILE_OPTIONS[number])
    ? PROFILE_OPTIONS
    : [preference.output_profile, ...PROFILE_OPTIONS];
  const outputControl: InspectorTreeNode = {
    id: `${node.id}.output`, label: 'Output Rime.Q',
    control: <input aria-label={`${node.label} output Rime.Q`} type="checkbox" disabled={controlsDisabled} checked={effectiveOutput} onChange={(event) => update('output_enabled', event.target.checked)} />,
  };
  if (!preference.output_enabled) {
    return [
      { id: `${node.id}.mode`, label: 'Mode', value: node.mode },
      { id: `${node.id}.status`, label: 'Status', value: 'disabled' },
      outputControl,
    ];
  }
  return [
    { id: `${node.id}.mode`, label: 'Mode', value: node.mode },
    { id: `${node.id}.status`, label: 'Status', value: effectiveOutput ? 'enabled' : 'disabled' },
    outputControl,
    {
      id: `${node.id}.profile`, label: 'Output profile',
      control: <select aria-label={`${node.label} output profile`} disabled={controlsDisabled} value={preference.output_profile} onChange={(event) => update('output_profile', event.target.value)}>{profileOptions.map((profile) => <option key={profile} value={profile}>{profile}</option>)}</select>,
    },
    {
      id: `${node.id}.dither`, label: 'Dither',
      control: <input aria-label={`${node.label} dither`} type="checkbox" disabled={controlsDisabled || !effectiveOutput} checked={effectiveDither} onChange={(event) => update('dither_enabled', event.target.checked)} />,
    },
    {
      id: `${node.id}.clip`, label: 'ClipType',
      control: <select aria-label={`${node.label} ClipType`} disabled={controlsDisabled} value={preference.clip_type} onChange={(event) => update('clip_type', event.target.value as ModuleQuantizationPreference['clip_type'])}>{CLIP_OPTIONS.map((clip) => <option key={clip} value={clip}>{clip}</option>)}</select>,
    },
  ];
}

function graphTreeNodes(
  parentId: string,
  config: GraphQuantizationConfig,
  canConfigure: boolean,
  onModuleChange: (moduleId: string, preference: ModuleQuantizationPreference) => void,
): readonly InspectorTreeNode[] {
  return normalGraphPresentation.nodes.filter((node) => node.parent_id === parentId).map((node) => {
    const children = graphTreeNodes(node.id, config, canConfigure, onModuleChange);
    const preference = node.execution_node_id === null ? undefined : config.modules.find((module) => module.module_id === node.execution_node_id);
    const quantizationChildren = preference === undefined
      ? []
      : moduleControls(node, preference, config, canConfigure, (next) => onModuleChange(preference.module_id, next));
    return { id: `graph.${node.id}`, label: node.label, value: node.mode, defaultExpanded: true, children: [...children, ...quantizationChildren] };
  });
}

function GraphInspector({ config, canConfigure, onGraphChange, onModuleChange }: {
  readonly config: GraphQuantizationConfig;
  readonly canConfigure: boolean;
  readonly onGraphChange: (config: GraphQuantizationConfig) => void;
  readonly onModuleChange: (moduleId: string, preference: ModuleQuantizationPreference) => void;
}) {
  const groups: readonly InspectorTreeGroup[] = [
    {
      id: 'overall', label: 'Overall', defaultExpanded: true,
      children: [{ id: 'overall.rimeq', label: 'Rime.Q', control: <input aria-label="Overall Rime.Q" aria-checked={config.enabled} role="switch" type="checkbox" disabled={!canConfigure} checked={config.enabled} onChange={(event) => onGraphChange({ ...config, enabled: event.target.checked })} /> }],
    },
    { id: 'graph', label: 'Hierarchy', defaultExpanded: true, children: graphTreeNodes(normalGraphPresentation.root_id, config, canConfigure, onModuleChange) },
  ];
  return <InspectorTree ariaLabel="Normal Graph inspector" groups={groups} storageKey="rime:graph-inspector:normal" />;
}

export function NodeInspector({ nodeId, envelope, dngFrame, frameCount, activeMethod, parameterValues, quantization = defaultQuantization, onMethodChange, onParameterChange, onGraphQuantizationChange = () => undefined, onModuleQuantizationChange = () => undefined }: NodeInspectorProps) {
  const canConfigure = envelope.lifecycleState === 'stop' || envelope.lifecycleState === 'completed';
  if (nodeId === null) {
    return <aside className="panel inspector-panel" aria-labelledby="inspector-heading"><div className="panel-heading compact"><div><span className="section-label">Graph inspector</span><h2 id="inspector-heading">Normal Graph</h2></div><span className="tree-mode-badge mode-enabled">graph</span></div><GraphInspector config={quantization} canConfigure={canConfigure} onGraphChange={onGraphQuantizationChange} onModuleChange={onModuleQuantizationChange} /></aside>;
  }

  const treeNode = normalGraphPresentation.nodes.find((node) => node.id === nodeId || node.execution_node_id === nodeId) ?? normalGraphPresentation.nodes[0];
  const executionNode = treeNode.execution_node_id === null ? undefined : normalManifest.nodes.find((node) => node.id === treeNode.execution_node_id);
  const output = executionNode?.outputs[0];
  const input = executionNode?.inputs[0];
  const backend = executionNode?.shader_entry === null ? 'asset' : executionNode ? 'wgsl' : 'none';
  const selectedMethod = executionNode?.methods.find((method) => method.method === activeMethod) ?? executionNode?.methods.find((method) => method.method === executionNode.default_method);
  const methodControl = executionNode === undefined || executionNode.methods.length === 0 ? undefined : <select aria-label={`${executionNode.id} method`} disabled={!canConfigure} value={selectedMethod?.method ?? executionNode.default_method} onChange={(event) => onMethodChange(executionNode.id, event.target.value)}>{executionNode.methods.map((method) => <option key={method.method} value={method.method}>{method.method} · {method.shader_entry.replace(/^demosaic_|_main$/g, '')}</option>)}</select>;
  const preference = executionNode === undefined ? undefined : quantization.modules.find((module) => module.module_id === executionNode.id);
  const groups: readonly InspectorTreeGroup[] = [
    { id: 'general', label: 'General', defaultExpanded: true, children: [{ id: 'general.mode', label: 'Mode', value: treeNode.mode }, { id: 'general.backend', label: 'Backend', value: backend }, { id: 'general.input', label: 'Input', value: input?.domain ?? 'unavailable' }, { id: 'general.output', label: 'Output', value: output?.domain ?? 'unavailable' }, { id: 'general.method', label: 'Method', value: selectedMethod?.method ?? '—', control: methodControl }, { id: 'general.shader', label: 'Shader', value: selectedMethod?.shader_entry ?? executionNode?.shader_entry ?? '—' }, { id: 'general.frame', label: 'Frame', value: `${envelope.frameIndex ?? '—'} / ${envelope.framePhase ?? 'idle'}` }, { id: 'general.runRevision', label: 'Run revision', value: String(envelope.runRevision) }, { id: 'general.methodRevision', label: 'Method revision', value: String(envelope.methodRevision) }, { id: 'general.configRevision', label: 'Config revision', value: String(envelope.configRevision) }, { id: 'general.gpuGeneration', label: 'GPU generation', value: String(envelope.gpuGeneration) }, ...(treeNode.reason === null ? [] : [{ id: 'general.reason', label: 'Reason', value: treeNode.reason }])] },
    { id: 'parameters', label: 'Parameters', defaultExpanded: true, children: selectedMethod === undefined ? [{ id: 'parameters.empty', label: 'Value', value: 'No parameters' }] : selectedMethod.parameters.map((parameter) => ({ id: `parameters.${parameter}`, label: parameter, value: String(parameterValues[parameter] ?? dngFrame?.cfa ?? '—'), control: parameter === 'cfa_pattern' || executionNode?.id !== 'dem' ? <output>{parameterValues[parameter] ?? dngFrame?.cfa ?? '—'}</output> : <input aria-label={parameter} disabled={!canConfigure} type="number" step="0.1" value={parameterValues[parameter] ?? ''} onChange={(event) => onParameterChange(executionNode.id, parameter, Number(event.target.value))} /> })) },
    ...(preference === undefined ? [] : [{ id: 'quantization', label: 'Rime.Q', defaultExpanded: true, children: moduleControls(treeNode, preference, quantization, canConfigure, (next) => onModuleQuantizationChange(preference.module_id, next)) }]),
  ];

  return <aside className="panel inspector-panel" aria-labelledby="inspector-heading"><div className="panel-heading compact"><div><span className="section-label">Node inspector</span><h2 id="inspector-heading">{treeNode.label}</h2></div><span className={`tree-mode-badge mode-${treeNode.mode}`}>{treeNode.mode}</span></div>{treeNode.id === 'raw_source' ? (dngFrame === null ? <div className="dng-empty-state"><strong>No DNG frame loaded</strong><span>Load a DNG to inspect the active frame metadata.</span></div> : <DngMetadataTree descriptor={dngFrame} lifecycleState={envelope.lifecycleState} frameIndex={envelope.frameIndex} frameCount={frameCount} />) : <><InspectorTree key={treeNode.id} ariaLabel={`${treeNode.label} inspector`} groups={groups} storageKey={`rime:node-inspector:${treeNode.id}`} /><div className="inspector-note">{treeNode.execution_node_id === null ? 'Architecture group; no executable runtime node.' : 'Mapped to the active Normal Graph runtime operator.'}</div></>}</aside>;
}
