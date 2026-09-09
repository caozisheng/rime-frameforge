import { describe, expect, it } from 'vitest';

import { DRC_PIPELINE_WGSL } from '../src/gpu/drc.js';
import { compileBlcShader, compileFusedNormalShader, compileSegmentedNormalShaders } from '../src/gpu/fused-normal-shader.js';

const bypassIds = [
  'sbpc_horizontal', 'dbpc', 'sbpc', 'raw_nr', 'tintless', 'lsc', 'cac', 'pfr', 'three_d_lut',
];

describe('fused Normal Graph WGSL compiler', () => {
  it('emits one fully fused compute entry for bilinear DEM', () => {
    const shader = compileFusedNormalShader('00');

    expect(shader.match(/@compute/g)).toHaveLength(1);
    expect(shader.match(/texture_storage_2d/g)).toHaveLength(5);
    expect(shader).toContain('texture_storage_2d<rgba16float');
    expect(shader).not.toContain('texture_storage_2d<rgba32float');
    expect(shader).toContain('textureLoad(drc_input');
    expect(shader).toContain('textureStore(yuv_output');
    expect(shader).toContain('fn sample_color_reproduce(p: vec2<i32>) -> vec4<f32> {');
    expect(shader).toContain('cr_sensor_to_prophoto(row: u32, col: u32)');
    expect(shader).toContain('cr_hs_lut: FloatBuffer');
    expect(shader).not.toContain('1.08 * rgb.r');
    const blc = compileBlcShader();
    expect(blc).toContain('textureStore(blc_output');
  });

  it('keeps complex DEM methods behind a bounded materialization boundary', () => {
    const shaders = compileSegmentedNormalShaders('02');

    expect(shaders.pre.match(/@compute/g)).toHaveLength(1);
    expect(shaders.dem.match(/@compute/g)).toHaveLength(1);
    expect(shaders.post.match(/@compute/g)).toHaveLength(1);
    expect(shaders.pre).toContain('pre_demosaic_main');
    expect(shaders.dem).toContain('demosaic_ppg_main');
    expect(shaders.post).toContain('postprocess_main');
    expect(shaders.pre.match(/texture_storage_2d/g)).toHaveLength(1);
    expect(shaders.quantize.match(/texture_storage_2d/g)).toHaveLength(1);
    expect(shaders.quantize).toContain('quantize_rgba(textureLoad(dem_input');
    expect(shaders.post.match(/texture_storage_2d/g)).toHaveLength(3);
    expect(shaders.post).toContain('return textureLoad(dem_input, p, 0);');
  });

  it('does not expose recursive complex DEM methods as fully fused shaders', () => {
    expect(() => compileFusedNormalShader('02')).toThrow('FUSED_GRAPH_BOUNDARY');
  });

  it('eliminates bypass operators and inlines the enabled pull chain', () => {
    const shader = compileFusedNormalShader('00');

    bypassIds.forEach((id) => expect(shader).not.toMatch(new RegExp(`fn (?:sample_)?${id}(?:\\(|_)`)));
    ['sample_wbc', 'sample_dem', 'sample_color_reproduce', 'sample_gamma', 'sample_rgb2yuv']
      .forEach((name) => expect(shader).toContain(`fn ${name}`));
  });

  it.each(['01', '02', '03', '04'] as const)('compiles DEM method %s in the bounded segmented path', (method) => {
    const shaders = compileSegmentedNormalShaders(method);

    expect(shaders.dem).toContain(`demosaic_${{ '01': 'mhc', '02': 'ppg', '03': 'vng', '04': 'ahd' }[method]}_main`);
    expect(shaders.pre).not.toContain('input_tex');
    expect(shaders.post).not.toContain('texture_storage_2d<r32float');
  });
  it('uses the materialized half-float output format for complex DEM methods', () => {
    const shaders = compileSegmentedNormalShaders('04');
    expect(shaders.dem).toContain('texture_storage_2d<rgba16float');
    expect(shaders.dem).not.toContain('texture_storage_2d<rgba32float');
  });

  it('derives WBC gains from the CFA channel instead of fixed pixel phase', () => {
    const shader = compileFusedNormalShader('00');

    expect(shader).toContain('let channel = params.cfa_pattern');
    expect(shader).toContain('let gain = params.white_balance_gains[channel]');
    expect(shader).not.toContain('gain = 2.0');
    expect(shader).not.toContain('gain = 1.5');
    expect(shader).toContain('let q = clamp_source(p)');
    expect(shader).toContain('clamp(quantize_scalar(textureLoad(drc_input, q, 0).x * gain, 1u, q), 0.0, module_saturation(1u))');
  });

  it('consumes the already white-balanced VFE input without analysis WBC', () => {
    expect(DRC_PIPELINE_WGSL).not.toContain('analysis_wbc_gains');
    expect(DRC_PIPELINE_WGSL).not.toContain('cfa_gain(position)');
    expect(DRC_PIPELINE_WGSL).toContain('load_zero(input_a, position).x * weights');
    expect(DRC_PIPELINE_WGSL).toContain('raw * clamp(target_value / luma');
  });

  it('clips every module output at the Rime.Q saturation boundary', () => {
    const shader = compileFusedNormalShader('00');
    const blc = compileBlcShader();

    const clipHelper = 'fn module_saturation(index: u32)';
    expect(blc).toContain(clipHelper);
    expect(shader).toContain(clipHelper);
    expect(blc).toContain('clamp(quantize_scalar((sample_raw(q) - params.black_level) / (params.white_level - params.black_level), 0u, q), 0.0, module_saturation(0u))');
    expect(shader).toContain('clamp(quantize_scalar(textureLoad(drc_input, q, 0).x * gain, 1u, q), 0.0, module_saturation(1u))');
    expect(DRC_PIPELINE_WGSL).toContain('clamp(raw * clamp(target_value / luma');
  });

  it('uses the gradient-guided DRC base path', () => {
    expect(DRC_PIPELINE_WGSL).toContain('gradient_guided');
    expect(DRC_PIPELINE_WGSL).toContain('gradient_chi');
    expect(DRC_PIPELINE_WGSL).toContain('GRADIENT_GUIDED_RADIUS_1');
    expect(DRC_PIPELINE_WGSL).toContain('gradient_weight');
  });

  it('selects saturation from quant_params qmax only when Rime.Q is enabled', () => {
    const blc = compileBlcShader();

    expect(blc).toContain('select(1.0, params.quant_params[index].qmax, quantization_enabled(index))');
  });

  it('embeds six inline Rime.Q output plans', () => {
    const shader = compileFusedNormalShader('00');

    expect(shader).toContain('quant_params: array<QuantParams, 6>');
    expect(shader.match(/quantize_(?:scalar|rgba)\(/g)?.length).toBeGreaterThanOrEqual(6);
  });
});
