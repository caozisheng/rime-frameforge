import { useRef, useState, type ChangeEvent, type ReactNode } from 'react';

import { CurveEditor } from './CurveEditor.js';
import { interpolateCurve, type CurvePoint } from './curve-model.js';
import { parseTuningProfile, serializeTuningProfile } from './profile-yaml.js';
import type { TuningControlKind } from './tuning-target.js';

interface TuningProfilePanelProps {
  readonly canConfigure: boolean;
  readonly parameter: 'ahd_l_threshold' | 'ahd_c_threshold_sq';
  readonly controlKind: TuningControlKind;
  readonly baseValues: Readonly<{ ahd_l_threshold?: string | number; ahd_c_threshold_sq?: string | number }>;
  readonly onApply: (parameter: 'ahd_l_threshold' | 'ahd_c_threshold_sq', value: number) => void;
  readonly onReset?: (parameter: 'ahd_l_threshold' | 'ahd_c_threshold_sq') => void;
}

const L_CURVE: readonly CurvePoint[] = [{ x: -4, y: 1.0 }, { x: 0, y: 1.05 }, { x: 4, y: 1.10 }, { x: 8, y: 1.16 }, { x: 12, y: 1.22 }, { x: 16, y: 1.28 }];
const C_CURVE: readonly CurvePoint[] = [{ x: -4, y: 3.0 }, { x: 0, y: 3.15 }, { x: 4, y: 3.30 }, { x: 8, y: 3.48 }, { x: 12, y: 3.66 }, { x: 16, y: 3.84 }];

export function resolveTuningParameterValue(_base: number, points: readonly CurvePoint[], x: number): number {
  return interpolateCurve(points, x, 'linear');
}

export function TuningProfilePanel({ canConfigure, parameter, controlKind, baseValues, onApply, onReset = () => undefined }: TuningProfilePanelProps): ReactNode {
  const [profileId, setProfileId] = useState('factory-default');
  const [profileName, setProfileName] = useState('Factory default');
  const [revision, setRevision] = useState(1);
  const [lCurve, setLCurve] = useState(L_CURVE);
  const [cCurve, setCCurve] = useState(C_CURVE);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const points = parameter === 'ahd_l_threshold' ? lCurve : cCurve;
  const onCurveChange = parameter === 'ahd_l_threshold' ? setLCurve : setCCurve;
  const base = Number(baseValues[parameter]);
  const resolvedValue = Number.isFinite(base) ? resolveTuningParameterValue(base, points, 4) : null;

  const apply = (): void => {
    if (resolvedValue === null) {
      setError(`IQ_PARAMETER_INVALID: ${parameter}`);
      return;
    }
    onApply(parameter, resolvedValue);
    setRevision((value) => value + 1);
    setError(null);
  };

  const reset = (): void => {
    onReset(parameter);
    setLCurve(L_CURVE);
    setCCurve(C_CURVE);
    setRevision((value) => value + 1);
    setError(null);
  };

  const save = (): void => {
    try {
      const yaml = serializeTuningProfile({ id: profileId, name: profileName, revision, lCurve, cCurve });
      const url = URL.createObjectURL(new Blob([yaml], { type: 'application/yaml' }));
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `${profileId || 'rime-tuning-profile'}.yaml`;
      anchor.click();
      URL.revokeObjectURL(url);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'IQ_PROFILE_SAVE_FAILED');
    }
  };

  const load = async (event: ChangeEvent<HTMLInputElement>): Promise<void> => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file === undefined) return;
    try {
      const loaded = parseTuningProfile(await file.text());
      setProfileId(loaded.id);
      setProfileName(loaded.name);
      setRevision(loaded.revision);
      setLCurve(loaded.lCurve);
      setCCurve(loaded.cCurve);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'IQ_PROFILE_LOAD_FAILED');
    }
  };

  const valueText = resolvedValue === null ? '—' : resolvedValue.toFixed(4);
  const baseText = Number.isFinite(base) ? base.toFixed(4) : '—';
  const axisText = parameter === 'ahd_l_threshold' || parameter === 'ahd_c_threshold_sq' ? 'scene_brightness_ev · EV100_scene' : 'scene_brightness_ev · EV100_scene';
  return <section className="iq-tuning-panel" aria-label="IQ Tuning" data-iq-parameter={parameter}><div className="iq-tuning-heading"><div><span className="section-label">IQ Tuning</span><strong>{parameter}</strong><small>{controlKind} · {profileId}</small></div><span className="tree-mode-badge mode-enabled">revision {revision}</span></div><div className="iq-tuning-columns"><div className="iq-tuning-controls"><div className="iq-tuning-meta"><span>Axis</span><strong>{axisText}</strong><span>Interpolation</span><strong>linear</strong></div><div className="iq-parameter-values" aria-label={`${parameter} values`}><div><span>Base LUT value</span><strong>{baseText}</strong></div><div><span>Current LUT value</span><strong>{valueText}</strong></div><div><span>Effect value</span><strong>{valueText}</strong></div></div><label>Profile name<input disabled={!canConfigure} value={profileName} onChange={(event) => setProfileName(event.target.value)} /></label><div className="iq-tuning-actions"><button disabled={!canConfigure} type="button" onClick={reset}>Reset to factory</button><button disabled={!canConfigure || resolvedValue === null} type="button" onClick={apply}>Apply</button><button disabled={!canConfigure || profileName.trim().length === 0} type="button" onClick={save}>Save profile</button><button disabled={!canConfigure} type="button" onClick={() => fileInput.current?.click()}>Load profile</button></div></div><div className="iq-tuning-curve"><h3>{parameter} · indexed LUT</h3><CurveEditor ariaLabel={`${parameter} curve`} disabled={!canConfigure} points={points} range={{ min: Math.min(...points.map((point) => point.y)), max: Math.max(...points.map((point) => point.y)) }} interpolation="linear" axisLabel="index / EV knot" valueLabel="parameter value" currentCoordinate={4} onChange={onCurveChange} /></div></div>{error === null ? null : <output className="iq-tuning-error">{error}</output>}<input accept=".yaml,.yml" hidden onChange={(event) => { void load(event); }} ref={fileInput} type="file" /></section>;
}
