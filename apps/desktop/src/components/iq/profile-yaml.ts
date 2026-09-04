import { parse, stringify } from 'yaml';

import type { CurvePoint } from './curve-model.js';

export interface TuningProfileDraft {
  readonly id: string;
  readonly name: string;
  readonly revision: number;
  readonly lCurve: readonly CurvePoint[];
  readonly cCurve: readonly CurvePoint[];
}

const ALL_MODULES: readonly [string, string][] = [
  ['vfe.blc', 'blc'], ['vfe.sbpc[0]', 'sbpc'], ['vfe.dbpc', 'dbpc'], ['vfe.sbpc[1]', 'sbpc'], ['vfe.tintless', 'tintless'], ['vfe.lsc', 'lsc'],
  ['vbe.hr', 'hr'], ['vbe.drc', 'drc'], ['vbe.cac', 'cac'], ['vbe.raw_nr', 'raw_nr'], ['vbe.wbc', 'wbc'], ['vbe.pfr', 'pfr'], ['vbe.ccm', 'ccm'], ['vbe.gamma', 'gamma'], ['vbe.3dlut', '3dlut'], ['vbe.rgb2yuv', 'rgb2yuv'],
  ['vpe.mctf[1]', 'mctf'], ['vpe.lce', 'lce'], ['vpe.ce', 'ce'], ['vpe.mctf[2]', 'mctf'], ['vpe.sharpen', 'sharpen'],
];

export function serializeTuningProfile(draft: TuningProfileDraft): string {
  const modules: Record<string, unknown> = Object.fromEntries(ALL_MODULES.map(([address, moduleId]) => [address, { module_id: moduleId, method: '00', tuning: 'unsupported' }]));
  modules['vbe.dem'] = {
    module_id: 'dem',
    method: '04',
    tuning: 'override',
    table: {
      schema_version: 1,
      parameter_schema_revision: 'dem04-v1',
      axes: [{ id: 'scene_brightness_ev', source: 'scene_meta.scene_brightness.ev_apex', unit: 'EV', knots: draft.lCurve.map((point) => point.x) }],
      effects: { ahd_l_threshold: { unit: 'lab_delta_l', values: draft.lCurve.map((point) => point.y) }, ahd_c_threshold_sq: { unit: 'lab_delta_ab_squared', values: draft.cCurve.map((point) => point.y) } },
      modulation_curves: [],
    },
  };
  return stringify({
    kind: 'rime.tuning_profile',
    schema_version: 1,
    profile: { id: draft.id, name: draft.name, profile_revision: draft.revision },
    pipeline: { graph_id: 'normal', manifest_revision: 'normal-v1', base_iq_set: 'factory-default' },
    camera: { profile_id: 'unknown', calibration_revision: 'unknown' },
    modules,
  });
}

interface SerializedProfile {
  readonly kind?: unknown;
  readonly profile?: { readonly id?: unknown; readonly name?: unknown; readonly profile_revision?: unknown };
  readonly modules?: Record<string, { readonly table?: { readonly axes?: readonly { readonly knots?: readonly number[] }[]; readonly effects?: { readonly ahd_l_threshold?: { readonly values?: readonly number[] }; readonly ahd_c_threshold_sq?: { readonly values?: readonly number[] } } } }>;
}

export function parseTuningProfile(source: string): TuningProfileDraft {
  const value = parse(source) as SerializedProfile;
  const demTable = value.modules?.['vbe.dem']?.table;
  const lValues = demTable?.effects?.ahd_l_threshold?.values;
  const cValues = demTable?.effects?.ahd_c_threshold_sq?.values;
  const knots = demTable?.axes?.[0]?.knots;
  if (value.kind !== 'rime.tuning_profile' || value.profile?.id === undefined || value.profile.name === undefined || lValues === undefined || cValues === undefined || knots === undefined || knots.length !== lValues.length || knots.length !== cValues.length) {
    throw new Error('IQ_PROFILE_INVALID: missing required profile fields');
  }
  return {
    id: String(value.profile.id),
    name: String(value.profile.name),
    revision: Number(value.profile.profile_revision ?? 1),
    lCurve: lValues.map((y, index) => ({ x: knots[index]!, y: Number(y) })),
    cCurve: cValues.map((y, index) => ({ x: knots[index]!, y: Number(y) })),
  };
}
