import { useRef, useState, type ChangeEvent, type ReactNode } from 'react';

import { CurveEditor } from './CurveEditor.js';
import { interpolateCurve, type CurvePoint } from './curve-model.js';
import { parseTuningProfile, serializeTuningProfile } from './profile-yaml.js';
import type { TuningControlKind } from './tuning-target.js';

export type TuningParameter = 'ahd_l_threshold' | 'ahd_c_threshold_sq' | 'gamma_lut' | 'drc_gain_offset_ev' | 'knee' | 'amplifier' | 'drc_edge_curve' | 'drc_luma_curve';

export interface TuningCurveDraft {
  readonly lCurve: readonly CurvePoint[];
  readonly cCurve: readonly CurvePoint[];
  readonly gammaCurve: readonly CurvePoint[];
  readonly drcEdgeCurve: readonly CurvePoint[];
  readonly drcLumaCurve: readonly CurvePoint[];
}

export const FACTORY_TUNING_CURVES: TuningCurveDraft = {
  lCurve: [{ x: -4, y: 1.0 }, { x: 0, y: 1.05 }, { x: 4, y: 1.10 }, { x: 8, y: 1.16 }, { x: 12, y: 1.22 }, { x: 16, y: 1.28 }],
  cCurve: [{ x: -4, y: 3.0 }, { x: 0, y: 3.15 }, { x: 4, y: 3.30 }, { x: 8, y: 3.48 }, { x: 12, y: 3.66 }, { x: 16, y: 3.84 }],
  gammaCurve: Array.from({ length: 9 }, (_, index) => ({ x: index / 8, y: index / 8 })),
  drcEdgeCurve: [{ x: 0, y: 1.0 }, { x: 0.1, y: 0.9 }, { x: 0.2, y: 0.7 }, { x: 0.3, y: 0.5 }, { x: 0.4, y: 0.3 }, { x: 0.5, y: 0.2 }, { x: 0.8, y: 0.0 }, { x: 1, y: 0.0 }],
  drcLumaCurve: [{ x: 0, y: 0.0 }, { x: 0.2, y: 0.3 }, { x: 0.3, y: 0.6 }, { x: 0.5, y: 0.7 }, { x: 0.7, y: 0.8 }, { x: 1, y: 1.0 }],
};

interface TuningProfilePanelProps {
  readonly canConfigure: boolean;
  readonly parameter: TuningParameter;
  readonly controlKind: TuningControlKind;
  readonly baseValues: Readonly<Record<string, string | number>>;
  readonly curves?: TuningCurveDraft;
  readonly onCurvesChange?: (curves: TuningCurveDraft) => void;
  readonly onApply: (parameter: TuningParameter, value: number) => void;
  readonly onLutApply?: (parameter: 'gamma_lut', values: readonly number[]) => void;
  readonly onReset?: (parameter: TuningParameter) => void;
  readonly onGammaLoad?: (gamma: number) => void;
}

function curveForParameter(curves: TuningCurveDraft, parameter: TuningParameter): readonly CurvePoint[] {
  if (parameter === 'ahd_l_threshold') return curves.lCurve;
  if (parameter === 'ahd_c_threshold_sq') return curves.cCurve;
  if (parameter === 'drc_edge_curve') return curves.drcEdgeCurve;
  if (parameter === 'drc_luma_curve') return curves.drcLumaCurve;
  return curves.gammaCurve;
}

function replaceCurve(curves: TuningCurveDraft, parameter: TuningParameter, points: readonly CurvePoint[]): TuningCurveDraft {
  if (parameter === 'ahd_l_threshold') return { ...curves, lCurve: points };
  if (parameter === 'ahd_c_threshold_sq') return { ...curves, cCurve: points };
  if (parameter === 'drc_edge_curve') return { ...curves, drcEdgeCurve: points };
  if (parameter === 'drc_luma_curve') return { ...curves, drcLumaCurve: points };
  return { ...curves, gammaCurve: points };
}

function monotoneGammaCurve(previous: readonly CurvePoint[], next: readonly CurvePoint[]): readonly CurvePoint[] {
  const changed = next.findIndex((point, index) => point.y !== previous[index]?.y);
  const minimum = previous[changed - 1]!.y;
  const maximum = previous[changed + 1]!.y;
  return next.map((point, index) => index === changed ? { ...point, y: Math.max(minimum, Math.min(maximum, point.y)) } : point);
}

export function resolveTuningParameterValue(_base: number, points: readonly CurvePoint[], x: number, interpolation: 'linear' | 'bezier' = 'linear'): number {
  return interpolateCurve(points, x, interpolation);
}
export function TuningProfilePanel({ canConfigure, parameter, controlKind, baseValues, curves = FACTORY_TUNING_CURVES, onCurvesChange = () => undefined, onApply, onLutApply = () => undefined, onReset = () => undefined, onGammaLoad = () => undefined }: TuningProfilePanelProps): ReactNode {
  const scalarDefault = parameter === 'drc_gain_offset_ev' ? 0 : 1;
  const [scalarDraft, setScalarDraft] = useState(() => Number.isFinite(Number(baseValues[parameter])) ? Number(baseValues[parameter]) : scalarDefault);
  const [profileId, setProfileId] = useState('factory-default');
  const [profileName, setProfileName] = useState('Factory default');
  const [revision, setRevision] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const isDrcCurve = parameter === 'drc_edge_curve' || parameter === 'drc_luma_curve';
  const isGammaLut = parameter === 'gamma_lut';
  const scalar = controlKind === 'scalar';
  const scalarConfig = parameter === 'drc_gain_offset_ev' ? { unit: 'EV', min: -4, max: 4, step: 0.1, range: '-4 to 4 EV' } : parameter === 'knee' ? { unit: 'normalized tone knee', min: 0, max: undefined, step: 0.01, range: '> 0 normalized' } : { unit: 'normalized amplifier', min: 0, max: undefined, step: 0.01, range: '≥ 0' };
  const scalarBase = Number.isFinite(Number(baseValues[parameter])) ? Number(baseValues[parameter]) : scalarDefault;
  const scalarValue = Number.isFinite(scalarDraft) ? scalarDraft : scalarBase;
  const scalarApply = (): void => {
    const valid = Number.isFinite(scalarValue) && (parameter !== 'knee' || scalarValue > 0) && scalarValue >= scalarConfig.min && (scalarConfig.max === undefined || scalarValue <= scalarConfig.max);
    if (!valid) { setError(`IQ_PARAMETER_INVALID: ${parameter}`); return; }
    onApply(parameter, scalarValue);
    setRevision((value) => value + 1);
    setError(null);
  };
  const scalarReset = (): void => {
    setScalarDraft(scalarDefault);
    onReset(parameter);
    setRevision((value) => value + 1);
    setError(null);
  };
  if (scalar) return <section className="iq-tuning-panel" aria-label="IQ Tuning" data-iq-parameter={parameter}>
    <div className="iq-tuning-heading"><div><span className="section-label">IQ Tuning</span><strong>{parameter}</strong><small>{controlKind} · factory-default</small></div><span className="tree-mode-badge mode-enabled">revision {revision}</span></div>
    <div className="iq-tuning-controls iq-scalar-controls">
      <div className="iq-tuning-meta"><span>Unit</span><strong>{scalarConfig.unit}</strong><span>Range</span><strong>{scalarConfig.range}</strong></div>
      <div className="iq-parameter-values" aria-label={`${parameter} values`}><div><span>Base value</span><strong>{Number.isFinite(scalarBase) ? scalarBase.toFixed(4) : '—'}</strong></div><div><span>Current value</span><strong>{Number.isFinite(scalarValue) ? scalarValue.toFixed(4) : '—'}</strong></div><div><span>Effect value</span><strong>{parameter === 'drc_gain_offset_ev' && Number.isFinite(scalarValue) ? (2 ** scalarValue).toFixed(4) : Number.isFinite(scalarValue) ? scalarValue.toFixed(4) : '—'}</strong></div></div>
      <label>{parameter}<input aria-label={`${parameter} current value`} disabled={!canConfigure} type="number" min={scalarConfig.min} max={scalarConfig.max} step={scalarConfig.step} value={Number.isFinite(scalarValue) ? scalarValue : ''} onChange={(event) => setScalarDraft(Number(event.target.value))} /></label>
      <div className="iq-tuning-actions"><button aria-label="Reset tuning to factory" disabled={!canConfigure} type="button" onClick={scalarReset}>↺</button><button aria-label="Apply tuning" disabled={!canConfigure || !Number.isFinite(scalarValue)} type="button" onClick={scalarApply}>✓</button></div>
    </div>
    {error === null ? null : <output className="iq-tuning-error">{error}</output>}
  </section>;
  const points = curveForParameter(curves, parameter);
  const interpolation = isGammaLut ? 'bezier' : 'linear';
  const currentCoordinate = isGammaLut || isDrcCurve ? 0.5 : 4;
  const base = isGammaLut || isDrcCurve ? 0.5 : Number(baseValues[parameter]);
  const resolvedValue = Number.isFinite(base) ? resolveTuningParameterValue(base, points, currentCoordinate, interpolation) : null;
  const range = isGammaLut || isDrcCurve ? { min: 0, max: 1 } : { min: Math.min(...points.map((point) => point.y)), max: Math.max(...points.map((point) => point.y)) };
  const onCurveChange = (next: readonly CurvePoint[]): void => {
    const constrained = isGammaLut ? monotoneGammaCurve(points, next) : next;
    onCurvesChange(replaceCurve(curves, parameter, constrained));
  };
  const apply = (): void => {
    if (resolvedValue === null) { setError(`IQ_PARAMETER_INVALID: ${parameter}`); return; }
    if (isGammaLut) onLutApply('gamma_lut', points.map((point) => point.y));
    else onApply(parameter, resolvedValue);
    setRevision((value) => value + 1);
    setError(null);
  };
  const reset = (): void => {
    onReset(parameter);
    onCurvesChange(replaceCurve(curves, parameter, curveForParameter(FACTORY_TUNING_CURVES, parameter)));
    setRevision((value) => value + 1);
    setError(null);
  };
  const save = (): void => {
    try {
      const yaml = serializeTuningProfile({ id: profileId, name: profileName, revision, gamma: Number(baseValues.gamma ?? 2.2), gammaCurve: curves.gammaCurve, lCurve: curves.lCurve, cCurve: curves.cCurve });
      const url = URL.createObjectURL(new Blob([yaml], { type: 'application/yaml' }));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `${profileId || 'rime-tuning-profile'}.yaml`;
      anchor.click();
      URL.revokeObjectURL(url);
      setError(null);
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'IQ_PROFILE_SAVE_FAILED'); }
  };
  const load = async (event: ChangeEvent<HTMLInputElement>): Promise<void> => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file === undefined) return;
    try {
      const loaded = parseTuningProfile(await file.text());
      onCurvesChange({ lCurve: loaded.lCurve, cCurve: loaded.cCurve, gammaCurve: loaded.gammaCurve ?? FACTORY_TUNING_CURVES.gammaCurve, drcEdgeCurve: curves.drcEdgeCurve, drcLumaCurve: curves.drcLumaCurve });
      if (loaded.gamma !== undefined) onGammaLoad(loaded.gamma);
      setError(null);
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'IQ_PROFILE_LOAD_FAILED'); }
  };
  const valueText = resolvedValue === null ? '—' : resolvedValue.toFixed(4);
  const baseText = Number.isFinite(base) ? base.toFixed(4) : '—';
  const axisText = isGammaLut ? 'linear_luminance_y · normalized' : isDrcCurve ? (parameter === 'drc_edge_curve' ? 'sobel_gradient_magnitude · normalized' : 'analysis_luma · normalized') : 'scene_brightness_ev · EV100_scene';
  const interpolationText = isGammaLut ? 'monotone Bézier' : 'linear';
  return <section className="iq-tuning-panel" aria-label="IQ Tuning" data-iq-parameter={parameter}>
    <div className="iq-tuning-heading"><div><span className="section-label">IQ Tuning</span><strong>{parameter}</strong><small>{controlKind} · {profileId}</small></div><span className="tree-mode-badge mode-enabled">revision {revision}</span></div>
    <div className="iq-tuning-columns"><div className="iq-tuning-controls">
      <div className="iq-tuning-meta"><span>Axis</span><strong>{axisText}</strong><span>Interpolation</span><strong>{interpolationText}</strong></div>
      <div className="iq-parameter-values" aria-label={`${parameter} values`}><div><span>Base LUT value</span><strong>{baseText}</strong></div><div><span>Current LUT value</span><strong>{valueText}</strong></div><div><span>Effect value</span><strong>{valueText}</strong></div></div>
      <label>Profile name<input disabled={!canConfigure} value={profileName} onChange={(event) => setProfileName(event.target.value)} /></label>
      <div className="iq-tuning-actions"><button aria-label="Reset tuning to factory" disabled={!canConfigure} type="button" onClick={reset}>↺</button><button aria-label="Apply tuning" disabled={!canConfigure || resolvedValue === null} type="button" onClick={apply}>✓</button><button aria-label="Save profile" disabled={!canConfigure || profileName.trim().length === 0} type="button" onClick={save}>⇩</button><button aria-label="Load profile" disabled={!canConfigure} type="button" onClick={() => fileInput.current?.click()}>⇧</button></div>
    </div><div className="iq-tuning-curve"><h3>{parameter} · indexed LUT</h3><CurveEditor ariaLabel={`${parameter} curve`} disabled={!canConfigure} points={points} range={range} interpolation={interpolation} axisLabel={isGammaLut ? 'index / linear Y knot' : isDrcCurve ? 'normalized input' : 'index / EV knot'} valueLabel={isGammaLut ? 'mapped luminance Y' : isDrcCurve ? 'modulation weight' : 'parameter value'} currentCoordinate={currentCoordinate} lockedPointIndices={isGammaLut ? [0, points.length - 1] : []} onChange={onCurveChange} /></div></div>
    {error === null ? null : <output className="iq-tuning-error">{error}</output>}<input accept=".yaml,.yml" hidden onChange={(event) => { void load(event); }} ref={fileInput} type="file" />
  </section>;
}
