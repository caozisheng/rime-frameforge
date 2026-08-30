import { normalGraphPresentation } from '../generated/normal_graph.generated.js';
import { normalGraphQuantization } from '../generated/normal_quantization.generated.js';
export type QuantizationClipType = 'truncate' | 'round' | 'dither' | 'dither_gpu';

export interface QuantizationModuleConfig {
  readonly module_id: string;
  readonly output_enabled: boolean;
  readonly output_profile: string;
  readonly clip_type: QuantizationClipType;
}

export interface QuantizationConfig {
  readonly graph_id: string;
  readonly enabled: boolean;
  readonly modules: readonly QuantizationModuleConfig[];
}

export interface GpuQuantizationPlan {
  readonly moduleId: string;
  readonly outputEnabled: boolean;
  readonly scale: number;
  readonly qmin: number;
  readonly qmax: number;
  readonly roundingMode: number;
  readonly seed: number;
  readonly streamId: number;
  readonly frameIndex: number;
  readonly plane: number;
  readonly ppc: number;
  readonly groupsPerRow: number;
  readonly groupsPerFrame: number;
}

const ROUNDING_MODE = {
  truncate: 0,
  round: 1,
  dither: 2,
  dither_gpu: 3,
} as const satisfies Record<QuantizationClipType, number>;
const PROFILE_PATTERN = /^([us])(\d+)\.(\d+)$/;
const DEFAULT_SEED = 0x1a5b6cfd;
const DEFAULT_PPC = 1;

export function buildGpuQuantizationPlans(
  config: QuantizationConfig,
  width: number,
  height: number,
  frameIndex = 0,
  graphEnabled: boolean = config.enabled,
): ReadonlyMap<string, GpuQuantizationPlan> {
  if (config.graph_id !== 'normal') throw new Error(`QUANTIZATION_GRAPH_INVALID: ${config.graph_id}`);
  const groupsPerRow = Math.ceil(width / DEFAULT_PPC);
  const groupsPerFrame = groupsPerRow * height;
  const plans = new Map<string, GpuQuantizationPlan>();

  config.modules.forEach((module, index) => {
    const match = PROFILE_PATTERN.exec(module.output_profile);
    if (match === null) throw new Error(`QUANTIZATION_PROFILE_INVALID: ${module.output_profile}`);
    const signed = match[1] === 's';
    const intBits = Number(match[2]);
    const fracBits = Number(match[3]);
    if (!Number.isSafeInteger(intBits) || !Number.isSafeInteger(fracBits) || intBits + fracBits > 24) {
      throw new Error(`QUANTIZATION_PROFILE_INVALID: ${module.output_profile}`);
    }

    const scale = 2 ** fracBits;
    const lsb = 1 / scale;
    const presentationNode = normalGraphPresentation.nodes.find((node) => node.execution_node_id === module.module_id);
    const moduleEnabled = presentationNode?.mode === 'enabled';
    plans.set(module.module_id, {
      moduleId: module.module_id,
      outputEnabled: graphEnabled && module.output_enabled && moduleEnabled,
      scale,
      qmin: signed ? -(2 ** intBits) : 0,
      qmax: 2 ** intBits - lsb,
      roundingMode: ROUNDING_MODE[module.clip_type],
      seed: DEFAULT_SEED,
      streamId: index + 1,
      frameIndex,
      plane: 0,
      ppc: DEFAULT_PPC,
      groupsPerRow,
      groupsPerFrame,
    });
  });
  return plans;
}

export const defaultQuantizationConfig: QuantizationConfig = normalGraphQuantization;
