import { useState, type ReactNode } from 'react';

import type { RuntimeEnvelope } from '../../../../web/src/contracts.js';
import { normalGraphPresentation } from '../../../../web/src/generated/normal_graph.generated.js';
import { normalManifest } from '../../../../web/src/generated/normal_manifest.generated.js';
import { normalGraphQuantization } from '../../../../web/src/generated/normal_quantization.generated.js';
import type { DngFrameDescriptor, DngSequenceDescriptor } from '../runtime/worker-bridge.js';
import { DngMetadataTree } from './DngMetadataTree.js';
import { InspectorTree, type InspectorTreeGroup, type InspectorTreeNode } from './InspectorTree.js';
import { tuningDescriptor, type TuningTarget } from './iq/tuning-target.js';
import { FACTORY_TUNING_CURVES, TuningProfilePanel, type TuningCurveDraft, type TuningParameter } from './iq/TuningProfilePanel.js';

export interface ModuleQuantizationPreference {
  readonly module_id: string;
  readonly output_enabled: boolean;
  readonly output_profile: string;
  readonly clip_type: 'truncate' | 'round' | 'dither' | 'dither_gpu';
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
  readonly dngSequence?: DngSequenceDescriptor | null;
  readonly frameCount: number;
  readonly activeMethod: string;
  readonly parameterValues: Readonly<Record<string, string | number>>;
  readonly appliedParameterValues?: Readonly<Record<string, string | number>>;
  readonly quantization?: GraphQuantizationConfig;
  readonly tuningCurves?: TuningCurveDraft;
  readonly onTuningCurvesChange?: (curves: TuningCurveDraft) => void;
  readonly onMethodChange: (nodeId: string, method: string) => void;
  readonly onParameterChange: (nodeId: string, parameter: string, value: number) => void;
  readonly onParameterApply?: (nodeId: string, parameter: string, value: number) => void;
  readonly onLutApply?: (nodeId: string, parameter: 'gamma_lut', values: readonly number[]) => void;
  readonly onParameterReset?: (nodeId: string, parameter: string) => void;
  readonly onGraphQuantizationChange?: (config: GraphQuantizationConfig) => void;
  readonly onModuleQuantizationChange?: (moduleId: string, preference: ModuleQuantizationPreference) => void;
}

type PresentationNode = (typeof normalGraphPresentation.nodes)[number];
const defaultQuantization: GraphQuantizationConfig = normalGraphQuantization;
const PROFILE_OPTIONS = ['s0.10', 's0.12', 's0.14', 'u0.10', 'u0.12', 'u0.14'] as const;
const CLIP_OPTIONS: readonly [ModuleQuantizationPreference['clip_type'], string][] = [['truncate', 'truncate'], ['round', 'round'], ['dither', 'dither'], ['dither_gpu', 'dither-gpu']];

function ToggleSwitch({ label, checked, disabled, onToggle }: { readonly label: string; readonly checked: boolean; readonly disabled: boolean; readonly onToggle: (checked: boolean) => void }): ReactNode {
  return <button aria-checked={checked} aria-label={label} className={`inspector-switch${checked ? ' is-on' : ''}`} disabled={disabled} role="switch" type="button" onClick={() => onToggle(!checked)}><span className="inspector-switch-track"><span className="inspector-switch-thumb" /></span></button>;
}

function moduleControls(node: PresentationNode, preference: ModuleQuantizationPreference, config: GraphQuantizationConfig, canConfigure: boolean, onChange: (preference: ModuleQuantizationPreference) => void): readonly InspectorTreeNode[] {
  const forcedOff = !config.enabled || node.mode === 'disabled' || node.mode === 'bypass';
  const controlsDisabled = !canConfigure || forcedOff;
  const effectiveOutput = !forcedOff && preference.output_enabled;
  const update = <K extends keyof ModuleQuantizationPreference>(key: K, value: ModuleQuantizationPreference[K]): void => onChange({ ...preference, [key]: value });
  const profileOptions = PROFILE_OPTIONS.includes(preference.output_profile as typeof PROFILE_OPTIONS[number]) ? PROFILE_OPTIONS : [preference.output_profile, ...PROFILE_OPTIONS];
  const outputControl: InspectorTreeNode = { id: `${node.id}.output`, label: 'Output Rime.Q', control: <ToggleSwitch label={`${node.label} output Rime.Q`} disabled={controlsDisabled} checked={effectiveOutput} onToggle={(checked) => update('output_enabled', checked)} /> };
  if (!preference.output_enabled) return [{ id: `${node.id}.mode`, label: 'Mode', value: node.mode }, { id: `${node.id}.status`, label: 'Status', value: 'disabled' }, outputControl];
  return [{ id: `${node.id}.mode`, label: 'Mode', value: node.mode }, { id: `${node.id}.status`, label: 'Status', value: effectiveOutput ? 'enabled' : 'disabled' }, outputControl, { id: `${node.id}.profile`, label: 'Output profile', control: <select aria-label={`${node.label} output profile`} disabled={controlsDisabled} value={preference.output_profile} onChange={(event) => update('output_profile', event.target.value)}>{profileOptions.map((profile) => <option key={profile} value={profile}>{profile}</option>)}</select> }, { id: `${node.id}.clip`, label: 'ClipType', control: <select aria-label={`${node.label} ClipType`} disabled={controlsDisabled} value={preference.clip_type} onChange={(event) => update('clip_type', event.target.value as ModuleQuantizationPreference['clip_type'])}>{CLIP_OPTIONS.map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select> }];
}

function graphTreeNodes(parentId: string, config: GraphQuantizationConfig, canConfigure: boolean, onModuleChange: (moduleId: string, preference: ModuleQuantizationPreference) => void): readonly InspectorTreeNode[] {
  return normalGraphPresentation.nodes.filter((node) => node.parent_id === parentId).map((node) => {
    const children = graphTreeNodes(node.id, config, canConfigure, onModuleChange);
    const preference = node.execution_node_id === null ? undefined : config.modules.find((module) => module.module_id === node.execution_node_id);
    const quantizationChildren = preference === undefined ? [] : moduleControls(node, preference, config, canConfigure, (next) => onModuleChange(preference.module_id, next));
    return { id: `graph.${node.id}`, label: node.label, value: node.mode, defaultExpanded: true, children: [...children, ...quantizationChildren] };
  });
}

function GraphInspector({ config, canConfigure, onGraphChange, onModuleChange }: { readonly config: GraphQuantizationConfig; readonly canConfigure: boolean; readonly onGraphChange: (config: GraphQuantizationConfig) => void; readonly onModuleChange: (moduleId: string, preference: ModuleQuantizationPreference) => void }): ReactNode {
  const groups: readonly InspectorTreeGroup[] = [{ id: 'overall', label: 'Overall', defaultExpanded: true, children: [{ id: 'overall.rimeq', label: 'Rime.Q', control: <ToggleSwitch label="Overall Rime.Q" disabled={!canConfigure} checked={config.enabled} onToggle={(checked) => onGraphChange({ ...config, enabled: checked })} /> }] }, { id: 'graph', label: 'Hierarchy', defaultExpanded: true, children: graphTreeNodes(normalGraphPresentation.root_id, config, canConfigure, onModuleChange) }];
  return <InspectorTree ariaLabel="Normal Graph inspector" groups={groups} storageKey="rime:graph-inspector:normal" />;
}

function parameterValue(parameter: string, parameterValues: Readonly<Record<string, string | number>>, dngFrame: DngFrameDescriptor | null): string | number {
  if (dngFrame !== null) {
    if (parameter === 'cfa_pattern') return dngFrame.cfa;
    const gainIndex = { red_gain: 0, green_gain: 1, blue_gain: 2 }[parameter];
    if (gainIndex !== undefined) return dngFrame.whiteBalanceGains[gainIndex] ?? '—';
  }
  return parameterValues[parameter] ?? '—';
}

export function NodeInspector({ nodeId, envelope, dngFrame, dngSequence = null, frameCount, activeMethod, parameterValues, appliedParameterValues = parameterValues, quantization = defaultQuantization, tuningCurves = FACTORY_TUNING_CURVES, onTuningCurvesChange = () => undefined, onMethodChange, onParameterChange, onParameterApply = onParameterChange, onLutApply = () => undefined, onParameterReset = () => undefined, onGraphQuantizationChange = () => undefined, onModuleQuantizationChange = () => undefined }: NodeInspectorProps): ReactNode {
  const [tuningTarget, setTuningTarget] = useState<TuningTarget | null>(null);
  const canConfigure = envelope.lifecycleState === 'stop' || envelope.lifecycleState === 'completed';
  if (nodeId === null) return <aside className="panel inspector-panel" aria-labelledby="inspector-heading"><div className="panel-heading compact"><div><span className="section-label">Graph inspector</span><h2 id="inspector-heading">Normal Graph</h2></div><span className="tree-mode-badge mode-enabled">graph</span></div><GraphInspector config={quantization} canConfigure={canConfigure} onGraphChange={onGraphQuantizationChange} onModuleChange={onModuleQuantizationChange} /></aside>;
  const treeNode = normalGraphPresentation.nodes.find((node) => node.id === nodeId || node.execution_node_id === nodeId) ?? normalGraphPresentation.nodes[0];
  const executionNode = treeNode.execution_node_id === null ? undefined : normalManifest.nodes.find((node) => node.id === treeNode.execution_node_id);
  const output = executionNode?.outputs[0];
  const input = executionNode?.inputs[0];
  const backend = executionNode?.shader_entry === null ? 'asset' : executionNode ? 'wgsl' : 'none';
  const selectedMethod = executionNode?.methods.find((method) => method.method === activeMethod) ?? executionNode?.methods.find((method) => method.method === executionNode.default_method);
  const methodControl = executionNode === undefined || executionNode.methods.length === 0 ? undefined : <select aria-label={`${executionNode.id} method`} disabled={!canConfigure} value={selectedMethod?.method ?? executionNode.default_method} onChange={(event) => onMethodChange(executionNode.id, event.target.value)}>{executionNode.methods.map((method) => <option key={method.method} value={method.method}>{method.method} · {method.shader_entry.replace(/^demosaic_|_main$/g, '')}</option>)}</select>;
  if (treeNode.id === 'raw_source') return <aside className="panel inspector-panel" aria-labelledby="inspector-heading"><div className="panel-heading compact"><div><span className="section-label">Node inspector</span><h2 id="inspector-heading">{treeNode.label}</h2></div><span className={`tree-mode-badge mode-${treeNode.mode}`}>{treeNode.mode}</span></div>{dngFrame === null ? <div className="dng-empty-state"><strong>No DNG frame loaded</strong><span>Load a DNG to inspect the active frame metadata.</span></div> : <DngMetadataTree descriptor={dngFrame} sequence={dngSequence} lifecycleState={envelope.lifecycleState} frameIndex={envelope.frameIndex} frameCount={frameCount} />}</aside>;
  const preference = executionNode === undefined ? undefined : quantization.modules.find((module) => module.module_id === executionNode.id);
  const parameterChildren = selectedMethod === undefined ? [{ id: 'parameters.empty', label: 'Value', value: 'No parameters' }] : selectedMethod.parameters.map((parameter) => {
    const value = parameterValue(parameter, parameterValues, dngFrame);
    const appliedValue = appliedParameterValues[parameter];
    const editableScalar = executionNode?.id === 'dem' || (executionNode?.id === 'gamma' && parameter === 'gamma');
    const dirty = editableScalar && typeof value === 'number' && typeof appliedValue === 'number' && value !== appliedValue;
    const descriptor = executionNode === undefined ? null : tuningDescriptor(executionNode.id, selectedMethod.method, parameter);
    const tuningActions = descriptor === null ? null : <><button aria-label={`Tune ${descriptor.parameter}`} className="inspector-tune-button" disabled={!canConfigure} onClick={() => setTuningTarget({ moduleAddress: `vbe.${executionNode!.id}`, moduleId: executionNode!.id, method: selectedMethod.method, parameter: descriptor.parameter, controlKind: descriptor.controlKind })} type="button">Tune</button><button aria-label={`Reset ${descriptor.parameter} to factory`} className="inspector-reset-button" disabled={!canConfigure} onClick={() => onParameterReset(executionNode!.id, descriptor.parameter)} type="button">Reset</button></>;
    let editor: ReactNode;
    if (parameter === 'cfa_pattern') editor = <output>{value}</output>;
    else if (editableScalar) editor = <span className={`inspector-parameter-editor${dirty ? ' is-dirty' : ''}`} data-parameter-dirty={dirty ? parameter : undefined}><input aria-label={parameter} data-current-parameter-value={`${parameter}:${value}`} disabled={!canConfigure} type="number" min={parameter === 'gamma' ? 1.8 : undefined} max={parameter === 'gamma' ? 2.4 : undefined} step="0.1" value={value === '—' ? '' : value} onChange={(event) => onParameterChange(executionNode!.id, parameter, Number(event.target.value))} />{tuningActions}</span>;
    else if (descriptor !== null) editor = <span className="inspector-parameter-editor"><output>{value}</output>{tuningActions}</span>;
    else editor = <output>{value}</output>;
    return { id: `parameters.${parameter}`, label: parameter, value: String(value), control: editor };
  });
  const groups: readonly InspectorTreeGroup[] = [{ id: 'general', label: 'General', defaultExpanded: true, children: [{ id: 'general.mode', label: 'Mode', value: treeNode.mode }, { id: 'general.backend', label: 'Backend', value: backend }, { id: 'general.input', label: 'Input', value: input?.domain ?? 'unavailable' }, { id: 'general.output', label: 'Output', value: output?.domain ?? 'unavailable' }, { id: 'general.method', label: 'Method', value: selectedMethod?.method ?? '—', control: methodControl }, { id: 'general.shader', label: 'Shader', value: selectedMethod?.shader_entry ?? executionNode?.shader_entry ?? '—' }, { id: 'general.frame', label: 'Frame', value: `${envelope.frameIndex ?? '—'} / ${envelope.framePhase ?? 'idle'}` }, { id: 'general.runRevision', label: 'Run revision', value: String(envelope.runRevision) }, { id: 'general.methodRevision', label: 'Method revision', value: String(envelope.methodRevision) }, { id: 'general.configRevision', label: 'Config revision', value: String(envelope.configRevision) }, { id: 'general.gpuGeneration', label: 'GPU generation', value: String(envelope.gpuGeneration) }, ...(treeNode.reason === null ? [] : [{ id: 'general.reason', label: 'Reason', value: treeNode.reason }])] }, { id: 'parameters', label: 'Parameters', defaultExpanded: true, children: parameterChildren }, ...(preference === undefined ? [] : [{ id: 'quantization', label: 'Rime.Q', defaultExpanded: true, children: moduleControls(treeNode, preference, quantization, canConfigure, (next) => onModuleQuantizationChange(preference.module_id, next)) }])];
  const selectedTarget = tuningTarget !== null && tuningTarget.moduleId === executionNode?.id && tuningTarget.method === selectedMethod?.method ? tuningTarget : null;
  const parameterView = <div className="inspector-parameters-view"><InspectorTree key={treeNode.id} ariaLabel={`${treeNode.label} inspector`} groups={groups} storageKey={`rime:node-inspector:${treeNode.id}`} /></div>;
  const tuningView = selectedTarget === null ? parameterView : <div className="iq-tuning-viewport"><div className="iq-tuning-content"><div className="iq-tuning-back"><button aria-label="Back to Parameters" onClick={() => setTuningTarget(null)} type="button">← Parameters</button></div><TuningProfilePanel canConfigure={canConfigure} parameter={selectedTarget.parameter as TuningParameter} controlKind={selectedTarget.controlKind} baseValues={parameterValues} curves={tuningCurves} onCurvesChange={onTuningCurvesChange} onApply={(parameter, value) => onParameterApply(executionNode?.id ?? 'dem', parameter, value)} onLutApply={(parameter, values) => onLutApply(executionNode?.id ?? 'gamma', parameter, values)} onReset={(parameter) => onParameterReset(executionNode?.id ?? 'dem', parameter)} onGammaLoad={(gamma) => onParameterChange('gamma', 'gamma', gamma)} /></div></div>;
  return <aside className="panel inspector-panel" aria-labelledby="inspector-heading"><div className="panel-heading compact"><div><span className="section-label">Node inspector</span><h2 id="inspector-heading">{treeNode.label}</h2></div><span className={`tree-mode-badge mode-${treeNode.mode}`}>{treeNode.mode}</span></div><div className="inspector-view-shell">{tuningView}</div></aside>;
}
