import { describe, expect, it } from 'vitest';

import { normalGraphPresentation } from '../src/generated/normal_graph.generated.js';
import {
  BYPASS_EXCLUDED_MODULE_IDS,
  canUserBypassModule,
  defaultGraphBypassConfig,
  validateGraphBypassConfig,
} from '../src/gpu/bypass.js';

const eligibleModuleIds: readonly string[] = normalGraphPresentation.nodes
  .filter((node) => node.kind === 'operator' && node.execution_node_id !== null)
  .map((node) => String(node.execution_node_id))
  .filter((moduleId) => !BYPASS_EXCLUDED_MODULE_IDS.includes(moduleId as (typeof BYPASS_EXCLUDED_MODULE_IDS)[number]));

const defaultConfig = () => defaultGraphBypassConfig();

describe('Normal Graph bypass configuration', () => {
  it('builds runtime bypass state for every non-excluded operator', () => {
    expect(defaultConfig().modules.map((module) => module.module_id)).toEqual(eligibleModuleIds);
    expect(defaultConfig().modules.find((module) => module.module_id === 'drc')?.bypass).toBe(false);
    expect(defaultConfig().modules.find((module) => module.module_id === 'cac')?.bypass).toBe(true);
  });

  it('exposes switches for every non-excluded operator', () => {
    expect(canUserBypassModule('drc')).toBe(true);
    expect(canUserBypassModule('cac')).toBe(true);
    expect(canUserBypassModule('raw_nr')).toBe(true);
    expect(canUserBypassModule('blc')).toBe(false);
    expect(canUserBypassModule('unknown')).toBe(false);
  });

  it('accepts the exact default module set', () => {
    expect(() => validateGraphBypassConfig(defaultConfig())).not.toThrow();
  });

  it('rejects a config for another graph', () => {
    expect(() => validateGraphBypassConfig({ ...defaultConfig(), graph_id: 'top' })).toThrow('BYPASS_CONFIG_GRAPH_INVALID');
  });

  it('rejects duplicate module ids', () => {
    const config = defaultConfig();
    expect(() => validateGraphBypassConfig({
      ...config,
      modules: [...config.modules, config.modules[0]!],
    })).toThrow('BYPASS_CONFIG_DUPLICATE_MODULE');
  });

  it('rejects unknown module ids', () => {
    const config = defaultConfig();
    expect(() => validateGraphBypassConfig({
      ...config,
      modules: [...config.modules.slice(1), { module_id: 'unknown', bypass: false }],
    })).toThrow('BYPASS_CONFIG_MODULE_INVALID');
  });

  it('rejects missing or extra modules', () => {
    const config = defaultConfig();
    expect(() => validateGraphBypassConfig({ ...config, modules: config.modules.slice(1) })).toThrow('BYPASS_CONFIG_MODULE_SET_INVALID');
    expect(() => validateGraphBypassConfig({
      ...config,
      modules: [...config.modules, { module_id: 'unknown', bypass: false }],
    })).toThrow('BYPASS_CONFIG_MODULE_INVALID');
  });

  it('rejects excluded modules even when they replace an expected module', () => {
    const config = defaultConfig();
    expect(() => validateGraphBypassConfig({
      ...config,
      modules: [...config.modules.slice(1), { module_id: 'blc', bypass: true }],
    })).toThrow('BYPASS_CONFIG_MODULE_EXCLUDED');
  });
});
