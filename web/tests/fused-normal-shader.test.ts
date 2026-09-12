import { describe, expect, it } from 'vitest';

import { DRC_PIPELINE_WGSL } from '../src/gpu/drc.js';
import { compileBlcShader, compileFusedNormalShader, compileSegmentedNormalShaders } from '../src/gpu/fused-normal-shader.js';
import { wbcPipelineWgsl } from '../src/generated/wbc_pipeline.generated.js';

const bypassIds = [
  'sbpc_horizontal', 'dbpc', 'sbpc', 'raw_nr', 'tintless', 'lsc', 'cac', 'pfr', 'three_d_lut',
];

describe('fused Normal Graph WGSL compiler', () => {
  it('emits one fully fused compute entry for bilinear DEM', () => {
    const shader = compileFusedNormalShader();

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
    // BLC runs as its own module pass (single-source blc00.wgsl with
    // module-owned bindings); the fused pass must not embed it.
    const blc = compileBlcShader();
    expect(shader).not.toContain('blc_main');
    expect(blc).toContain('textureStore(output_tex');
    expect(blc).toContain('BlcParams');
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
    // The method boundary moved to Rust (generation time): the fused asset
    // embeds only the bilinear sampler, and the segmented table exposes
    // exactly methods 01-04 — '00' is a type-level absence, not a runtime
    // path.
    expect(compileFusedNormalShader()).toContain('dem00_sample');
    for (const method of ['01', '02', '03', '04'] as const) {
      expect(compileSegmentedNormalShaders(method).dem).not.toContain('dem00_sample');
    }
    expect(() => compileSegmentedNormalShaders('05' as never)).toThrow();
  });

  it('eliminates bypass operators and inlines the enabled pull chain', () => {
    const shader = compileFusedNormalShader();

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

    expect(shaders.dem).toContain('demosaic_ahd_main');
    expect(shaders.pre).not.toContain('input_tex');
    expect(shaders.post).not.toContain('texture_storage_2d<r32float');
  });

  it('derives WBC gains from the CFA channel instead of fixed pixel phase', () => {
    // WBC algorithm is single-sourced from rime-isp (wbc00.wgsl) via the
    // generated asset — consumed by the generic module shell with the
    // module's own bindings; no renaming adapter anywhere.
    const wbc = wbcPipelineWgsl;
    const fused = compileFusedNormalShader();
    expect(wbc).toContain('fn channel_at(position: vec2<i32>) -> u32 {');
    expect(wbc).toContain('params.gains[channel]');
    expect(wbc).toContain('fn divided_value(position: vec2<i32>) -> f32 {');
    expect(wbc).toContain('fn wbc_main(');
    // The FUSED post-DRC shader is the identity passthrough only — the
    // algorithm runs in the standalone pre-DRC wbc pass.
    expect(fused).toContain('fn sample_wbc(p: vec2<i32>) -> f32 {');
    expect(fused).not.toContain('fn dem_unclip(position');
    expect(fused).not.toContain('params.white_balance_gains[channel_at(p)] / params.hr_gain_enable.x');
  });

  it('serves BLC through the generic module shell from the single-source asset', async () => {
    const { ModuleShaderRuntime } = await import('../src/gpu/module-shader.js');
    const { blcPipelineWgsl } = await import('../src/generated/blc_pipeline.generated.js');
    expect(typeof ModuleShaderRuntime).toBe('function');
    // The single-source module shader declares module-owned bindings
    // (0 = uniform BlcParams, 1 = input_tex, 2 = output_tex) — the shell
    // binds resources verbatim, no fused-param adaptation.
    expect(blcPipelineWgsl).toContain('@group(0) @binding(0) var<uniform> params: BlcParams;');
    expect(blcPipelineWgsl).toContain('@group(0) @binding(1) var input_tex: texture_2d<u32>;');
    expect(blcPipelineWgsl).toContain('@group(0) @binding(2) var output_tex: texture_storage_2d<r32float, write>;');
    expect(blcPipelineWgsl).toContain('fn blc_main(');
  });

  it('consumes the already white-balanced VFE input without analysis WBC', () => {
    expect(DRC_PIPELINE_WGSL).not.toContain('analysis_wbc_gains');
    expect(DRC_PIPELINE_WGSL).not.toContain('cfa_gain(position)');
    expect(DRC_PIPELINE_WGSL).toContain('load_zero(input_a, position).x * weights');
    expect(DRC_PIPELINE_WGSL).toContain('raw * clamp(target_value / luma');
  });

  it('clips every module output at the Rime.Q saturation boundary', () => {
    const shader = compileFusedNormalShader();

    const clipHelper = 'fn module_saturation(index: u32)';
    expect(shader).toContain(clipHelper);
    expect(shader).toContain('clamp(quantize_scalar(');
    expect(wbcPipelineWgsl).toContain('fn wbc_sample(position: vec2<i32>, extent: vec2<i32>) -> f32 {');
    // Port clamp lives in the single-source module shader (s0.14 container).
    expect(wbcPipelineWgsl).toContain('min(wbc_sample(position, vec2<i32>(extent)), 1.0 - 1.0 / 16384.0)');
    expect(DRC_PIPELINE_WGSL).toContain('clamp(raw * clamp(target_value / luma');
  });

  it('uses the gradient-guided DRC base path', () => {
    expect(DRC_PIPELINE_WGSL).toContain('gradient_guided');
    expect(DRC_PIPELINE_WGSL).toContain('gradient_chi');
    expect(DRC_PIPELINE_WGSL).toContain('GRADIENT_GUIDED_RADIUS_1');
    expect(DRC_PIPELINE_WGSL).toContain('gradient_weight');
  });

  it('selects saturation from quant_params qmax only when Rime.Q is enabled', () => {
    const shader = compileFusedNormalShader();

    expect(shader).toContain('select(1.0, params.quant_params[index].qmax, quantization_enabled(index))');
  });

  it('embeds six inline Rime.Q output plans', () => {
    const shader = compileFusedNormalShader();

    expect(shader).toContain('quant_params: array<QuantParams, 6>');
    expect(shader.match(/quantize_(?:scalar|rgba)\(/g)?.length).toBeGreaterThanOrEqual(6);
  });

  it('keeps WBC highlight recovery as a single pass without scratch bindings', () => {
    // The algorithm is single-sourced from rime-isp wbc00.wgsl via the
    // generated asset (divided domain, MATLAB max/mean-by-evidence
    // recovery, two-sided delta feather); the generic module shell binds
    // the module's own resources verbatim.
    const src = wbcPipelineWgsl;
    expect(src).not.toContain('@group(0) @binding(8)');
    expect(src).not.toContain('fn wbc_hr_analysis_main');
    expect(src).not.toContain('fn wbc_hr_reduce_main');
    expect(src).toContain('fn wbc_sample(position: vec2<i32>, extent: vec2<i32>) -> f32 {');
    expect(src).toContain('fn dem_unclip(position: vec2<i32>, extent: vec2<i32>) -> vec3<f32> {');
    expect(src).toContain('fn recovered_divided(tex: vec3<f32>, weights: vec3<f32>, channel: u32) -> f32 {');
    expect(src).toContain('fn blurred_delta(position: vec2<i32>, extent: vec2<i32>) -> f32 {');
    // module-owned binding names survive verbatim (no adapter renaming)
    expect(src).toContain('textureLoad(input_tex, position, 0).r');
  });
});
