import { normalGraphPresentation } from '../generated/normal_graph.generated.js';

export interface GraphBypassModule {
  readonly module_id: string;
  readonly bypass: boolean;
}

export interface GraphBypassConfig {
  readonly graph_id: string;
  readonly modules: readonly GraphBypassModule[];
}

export const BYPASS_EXCLUDED_MODULE_IDS = [
  'blc',
  'wbc',
  'dem',
  'color_correction',
  'gamma',
  'rgb2yuv',
] as const;

const excludedModuleIds = new Set<string>(BYPASS_EXCLUDED_MODULE_IDS);
const userBypassModuleIds: readonly string[] = normalGraphPresentation.nodes
  .filter((node) => node.kind === 'operator' && node.execution_node_id !== null)
  .map((node) => String(node.execution_node_id))
  .filter((moduleId) => !excludedModuleIds.has(moduleId));
const userBypassModuleIdSet = new Set<string>(userBypassModuleIds);

export function canUserBypassModule(moduleId: string): boolean {
  return userBypassModuleIdSet.has(moduleId);
}

export function defaultGraphBypassConfig(): GraphBypassConfig {
  return {
    graph_id: normalGraphPresentation.graph_id,
    modules: userBypassModuleIds.map((module_id) => ({ module_id, bypass: module_id !== 'drc' })),
  };
}

export function validateGraphBypassConfig(config: GraphBypassConfig): void {
  if (config.graph_id !== normalGraphPresentation.graph_id) {
    throw new Error(`BYPASS_CONFIG_GRAPH_INVALID: ${config.graph_id}`);
  }

  const suppliedModuleIds = new Set<string>();
  for (const module of config.modules) {
    if (suppliedModuleIds.has(module.module_id)) {
      throw new Error(`BYPASS_CONFIG_DUPLICATE_MODULE: ${module.module_id}`);
    }
    suppliedModuleIds.add(module.module_id);

    if (excludedModuleIds.has(module.module_id)) {
      throw new Error(`BYPASS_CONFIG_MODULE_EXCLUDED: ${module.module_id}`);
    }
    if (!userBypassModuleIdSet.has(module.module_id)) {
      throw new Error(`BYPASS_CONFIG_MODULE_INVALID: ${module.module_id}`);
    }
  }

  if (suppliedModuleIds.size !== userBypassModuleIdSet.size || [...userBypassModuleIdSet].some((moduleId) => !suppliedModuleIds.has(moduleId))) {
    throw new Error('BYPASS_CONFIG_MODULE_SET_INVALID');
  }
}
