export type TuningControlKind = 'curve' | 'scalar' | 'matrix' | 'lut_1d' | 'lut_2d' | 'lut_3d' | 'table' | 'range' | 'enum' | 'custom';

export interface TuningTarget {
  readonly moduleAddress: string;
  readonly moduleId: string;
  readonly method: string;
  readonly parameter: string;
  readonly controlKind: TuningControlKind;
}

export interface TuningParameterDescriptor {
  readonly parameter: string;
  readonly controlKind: TuningControlKind;
}

export function tuningDescriptor(moduleId: string, method: string, parameter: string): TuningParameterDescriptor | null {
  if (moduleId === 'dem' && method === '04' && (parameter === 'ahd_l_threshold' || parameter === 'ahd_c_threshold_sq')) {
    return { parameter, controlKind: 'curve' };
  }
  if (moduleId === 'gamma' && method === '00' && parameter === 'gamma_lut') {
    return { parameter, controlKind: 'lut_1d' };
  }
  return null;
}
