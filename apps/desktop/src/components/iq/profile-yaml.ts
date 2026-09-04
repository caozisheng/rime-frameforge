import { parse, stringify } from 'yaml';

import type { CurvePoint } from './curve-model.js';

export interface TuningProfileDraft {
  readonly id: string;
  readonly name: string;
  readonly revision: number;
  readonly lCurve: readonly CurvePoint[];
  readonly cCurve: readonly CurvePoint[];
}

export function serializeTuningProfile(draft: TuningProfileDraft): string {
  return stringify({
    kind: 'rime.tuning_profile',
    schema_version: 1,
    profile: { id: draft.id, name: draft.name, profile_revision: draft.revision },
    pipeline: { graph_id: 'normal', base_iq_set: 'factory-default' },
    modules: {
      'vbe.dem': {
        module_id: 'dem',
        method: '04',
        tuning: 'override',
        modulation_curves: {
          ahd_l_threshold: draft.lCurve,
          ahd_c_threshold_sq: draft.cCurve,
        },
      },
    },
  });
}

export function parseTuningProfile(source: string): TuningProfileDraft {
  const value = parse(source) as { kind?: unknown; profile?: { id?: unknown; name?: unknown; profile_revision?: unknown }; modules?: Record<string, { modulation_curves?: { ahd_l_threshold?: CurvePoint[]; ahd_c_threshold_sq?: CurvePoint[] } }> };
  const curves = value.modules?.['vbe.dem']?.modulation_curves;
  if (value.kind !== 'rime.tuning_profile' || value.profile?.id === undefined || value.profile.name === undefined || curves?.ahd_l_threshold === undefined || curves.ahd_c_threshold_sq === undefined) {
    throw new Error('IQ_PROFILE_INVALID: missing required profile fields');
  }
  return {
    id: String(value.profile.id),
    name: String(value.profile.name),
    revision: Number(value.profile.profile_revision ?? 1),
    lCurve: curves.ahd_l_threshold,
    cCurve: curves.ahd_c_threshold_sq,
  };
}
