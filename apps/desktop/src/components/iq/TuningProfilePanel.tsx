import { useRef, useState, type ChangeEvent, type ReactNode } from 'react';

import { CurveEditor } from './CurveEditor.js';
import type { CurvePoint } from './curve-model.js';
import { parseTuningProfile, serializeTuningProfile } from './profile-yaml.js';

interface TuningProfilePanelProps {
  readonly canConfigure: boolean;
  readonly baseValues: Readonly<{ ahd_l_threshold?: string | number; ahd_c_threshold_sq?: string | number }>;
  readonly onApply: (parameter: 'ahd_l_threshold' | 'ahd_c_threshold_sq', value: number) => void;
}

const L_CURVE: readonly CurvePoint[] = [{ x: -4, y: 0.12 }, { x: 0, y: 0.05 }, { x: 4, y: 0 }, { x: 8, y: -0.03 }, { x: 12, y: -0.08 }];
const C_CURVE: readonly CurvePoint[] = [{ x: -4, y: 0.10 }, { x: 0, y: 0.04 }, { x: 4, y: 0 }, { x: 8, y: -0.02 }, { x: 12, y: -0.06 }];

function curveAt(points: readonly CurvePoint[], x: number): number {
  if (points.length === 0) return 0;
  if (x <= points[0]!.x) return points[0]!.y;
  for (let index = 1; index < points.length; index += 1) {
    const right = points[index]!;
    const left = points[index - 1]!;
    if (x <= right.x) {
      const amount = (x - left.x) / (right.x - left.x);
      return left.y + amount * (right.y - left.y);
    }
  }
  return points.at(-1)!.y;
}

export function TuningProfilePanel({ canConfigure, baseValues, onApply }: TuningProfilePanelProps): ReactNode {
  const [profileId, setProfileId] = useState('factory-default');
  const [profileName, setProfileName] = useState('Factory default');
  const [revision, setRevision] = useState(1);
  const [lCurve, setLCurve] = useState(L_CURVE);
  const [cCurve, setCCurve] = useState(C_CURVE);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  const apply = (): void => {
    const l = Number(baseValues.ahd_l_threshold);
    const c = Number(baseValues.ahd_c_threshold_sq);
    if (Number.isFinite(l)) onApply('ahd_l_threshold', l * 2 ** curveAt(lCurve, 4));
    if (Number.isFinite(c)) onApply('ahd_c_threshold_sq', c * 2 ** curveAt(cCurve, 4));
    setRevision((value) => value + 1);
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

  return <section className="iq-tuning-panel" aria-label="IQ Tuning"><div className="iq-tuning-heading"><div><span className="section-label">IQ Tuning</span><strong>{profileId}</strong></div><span className="tree-mode-badge mode-enabled">revision {revision}</span></div><label>Profile name<input disabled={!canConfigure} value={profileName} onChange={(event) => setProfileName(event.target.value)} /></label><div className="iq-tuning-card-grid"><article className="iq-tuning-card"><h3>ahd_l_threshold</h3><CurveEditor ariaLabel="ahd_l_threshold curve" disabled={!canConfigure} points={lCurve} range={{ min: -1, max: 1 }} onChange={setLCurve} /></article><article className="iq-tuning-card"><h3>ahd_c_threshold_sq</h3><CurveEditor ariaLabel="ahd_c_threshold_sq curve" disabled={!canConfigure} points={cCurve} range={{ min: -1, max: 1 }} onChange={setCCurve} /></article></div>{error === null ? null : <output className="iq-tuning-error">{error}</output>}<input accept=".yaml,.yml" hidden onChange={(event) => { void load(event); }} ref={fileInput} type="file" /><div className="iq-tuning-actions"><button disabled={!canConfigure} type="button" onClick={() => { setLCurve(L_CURVE); setCCurve(C_CURVE); }}>Reset</button><button disabled={!canConfigure} type="button" onClick={apply}>Apply</button><button disabled={!canConfigure || profileName.trim().length === 0} type="button" onClick={save}>Save profile</button><button disabled={!canConfigure} type="button" onClick={() => fileInput.current?.click()}>Load profile</button></div></section>;
}
