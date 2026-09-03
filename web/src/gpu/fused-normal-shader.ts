import { buildFusedGraphPlan } from './fused-graph-plan.js';
import quantizeShader from '../../../crates/rime-quant/shaders/quantize.wgsl?raw';
import demBilinearShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_00.wgsl?raw';
import demMhcShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_01.wgsl?raw';
import demPpgShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_02.wgsl?raw';
import demVngShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_03.wgsl?raw';
import demAhdShader from '../../../crates/rime-isp/src/vbe/dem/demosaic_04.wgsl?raw';

const DEMOSAIC_SHADERS = { '00': demBilinearShader, '01': demMhcShader, '02': demPpgShader, '03': demVngShader, '04': demAhdShader } as const;
const QUANTIZE_FUNCTIONS = quantizeShader.slice(0, quantizeShader.indexOf('@group(0)'));
const FUSED_PARAMS = `struct FusedParams {
  width: u32,
  height: u32,
  black_level: f32,
  white_level: f32,
  cfa_pattern: vec4<u32>,
  white_balance_gains: vec4<f32>,
  vng_threshold: f32,
  ahd_l_threshold: f32,
  ahd_c_threshold_sq: f32,
  frame_index: u32,
  quant_params: array<QuantParams, 6>,
  quant_enabled_0: vec4<u32>,
  quant_enabled_1: vec4<u32>,
}`;
const QUANT_HELPERS = `fn quantization_enabled(index: u32) -> bool {
  if (index < 4u) { return params.quant_enabled_0[index] != 0u; }
  return params.quant_enabled_1[index - 4u] != 0u;
}
fn quantize_scalar(value: f32, index: u32, p: vec2<i32>) -> f32 {
  if (!quantization_enabled(index)) { return value; }
  let pixel_group = u32(max(p.y, 0)) * params.width + u32(max(p.x, 0));
  return quantize_sample(value, params.quant_params[index], pixel_group, 0u);
}
fn quantize_rgba(value: vec4<f32>, index: u32, p: vec2<i32>) -> vec4<f32> {
  if (!quantization_enabled(index)) { return value; }
  let pixel_group = u32(max(p.y, 0)) * params.width + u32(max(p.x, 0));
  var quant = params.quant_params[index];
  quant.channel = 0u; let r = quantize_sample(value.r, quant, pixel_group, 0u);
  quant.channel = 1u; let g = quantize_sample(value.g, quant, pixel_group, 0u);
  quant.channel = 2u; let b = quantize_sample(value.b, quant, pixel_group, 0u);
  quant.channel = 3u; let a = quantize_sample(value.a, quant, pixel_group, 0u);
  return vec4<f32>(r, g, b, a);
}`;

export interface SegmentedNormalShaders {
  readonly pre: string;
  readonly dem: string;
  readonly quantize: string;
  readonly post: string;
}

export function compileFusedNormalShader(demMethod: keyof typeof DEMOSAIC_SHADERS = '00'): string {
  const plan = buildFusedGraphPlan();
  if (plan.nodes.length !== 6 || plan.previewNodeId !== 'rgb2yuv') throw new Error('FUSED_GRAPH_INVALID: unexpected Normal Graph plan');
  if (demMethod !== '00') {
    throw new Error(`FUSED_GRAPH_BOUNDARY: DEM method ${demMethod} requires a materialization boundary`);
  }
  const demosaic = adaptBilinearDemosaic(demBilinearShader);
  return `${QUANTIZE_FUNCTIONS}
// dem-method:00
${FUSED_PARAMS}
@group(0) @binding(0) var raw_input: texture_2d<u32>;
@group(0) @binding(1) var blc_output: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var wbc_output: texture_storage_2d<r32float, write>;
@group(0) @binding(3) var dem_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var color_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var gamma_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var yuv_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(7) var<uniform> params: FusedParams;
${QUANT_HELPERS}
${rawBlcWbcFunctions()}
${demosaic}
${postprocessFunctions('sample_dem(p)')}
@compute @workgroup_size(8, 8)
fn normal_fused_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(blc_output, p, vec4<f32>(sample_blc(p), 0.0, 0.0, 1.0));
  textureStore(wbc_output, p, vec4<f32>(sample_wbc(p), 0.0, 0.0, 1.0));
  textureStore(dem_output, p, sample_dem_quantized(p));
  textureStore(color_output, p, sample_color_correction(p));
  textureStore(gamma_output, p, sample_gamma(p));
  textureStore(yuv_output, p, sample_rgb2yuv(p));
}`;
}

export function compileSegmentedNormalShaders(demMethod: keyof typeof DEMOSAIC_SHADERS): SegmentedNormalShaders {
  const plan = buildFusedGraphPlan();
  if (plan.nodes.length !== 6 || plan.previewNodeId !== 'rgb2yuv') throw new Error('FUSED_GRAPH_INVALID: unexpected Normal Graph plan');
  if (demMethod === '00') throw new Error('FUSED_GRAPH_SEGMENT_INVALID: bilinear DEM should use full fusion');
  return {
    pre: compilePreShader(),
    dem: sanitizeDemosaicShader(DEMOSAIC_SHADERS[demMethod]),
    quantize: compileDemQuantizeShader(demMethod),
    post: compilePostShader(demMethod),
  };
}

function rawBlcWbcFunctions(): string {
  return `fn source_extent() -> vec2<u32> { return vec2<u32>(params.width, params.height); }
fn clamp_source(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2<i32>(0), vec2<i32>(source_extent()) - vec2<i32>(1)); }
fn sample_raw(p: vec2<i32>) -> f32 { return f32(textureLoad(raw_input, clamp_source(p), 0).r); }
fn sample_blc(p: vec2<i32>) -> f32 {
  let q = clamp_source(p);
  return quantize_scalar((sample_raw(q) - params.black_level) / (params.white_level - params.black_level), 0u, q);
}
fn sample_wbc(p: vec2<i32>) -> f32 {
  let q = clamp_source(p);
  let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u);
  let channel = params.cfa_pattern[phase.y * 2u + phase.x];
  let gain = params.white_balance_gains[channel];
  return quantize_scalar(sample_blc(q) * gain, 1u, q);
}`;
}

function postprocessFunctions(demExpression: string): string {
  return `fn sample_dem_quantized(p: vec2<i32>) -> vec4<f32> { return quantize_rgba(${demExpression}, 2u, p); }
fn sample_color_correction(p: vec2<i32>) -> vec4<f32> {
  let rgb = sample_dem_quantized(p).rgb;
  let corrected = vec4<f32>(1.08 * rgb.r - 0.04 * rgb.g - 0.04 * rgb.b, -0.03 * rgb.r + 1.06 * rgb.g - 0.03 * rgb.b, -0.02 * rgb.r - 0.06 * rgb.g + 1.08 * rgb.b, 1.0);
  return quantize_rgba(corrected, 3u, p);
}
fn sample_gamma(p: vec2<i32>) -> vec4<f32> {
  let rgb = max(sample_color_correction(p).rgb, vec3<f32>(0.0));
  return quantize_rgba(vec4<f32>(pow(rgb, vec3<f32>(1.0 / 2.2)), 1.0), 4u, p);
}
fn sample_rgb2yuv(p: vec2<i32>) -> vec4<f32> {
  let rgb = sample_gamma(p).rgb;
  let yuv = vec4<f32>(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), dot(rgb, vec3<f32>(-0.114572, -0.385428, 0.5)) + 0.5, dot(rgb, vec3<f32>(0.5, -0.454153, -0.045847)) + 0.5, 1.0);
  return quantize_rgba(yuv, 5u, p);
}`;
}

function compilePreShader(): string {
  return `${QUANTIZE_FUNCTIONS}
${FUSED_PARAMS}
@group(0) @binding(0) var raw_input: texture_2d<u32>;
@group(0) @binding(1) var blc_output: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var pre_output: texture_storage_2d<r32float, write>;
@group(0) @binding(3) var<uniform> params: FusedParams;
${QUANT_HELPERS}
${rawBlcWbcFunctions()}
@compute @workgroup_size(8, 8)
fn pre_demosaic_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(blc_output, p, vec4<f32>(sample_blc(p), 0.0, 0.0, 1.0));
  textureStore(pre_output, p, vec4<f32>(sample_wbc(p), 0.0, 0.0, 1.0));
}`;
}

function compileDemQuantizeShader(method: keyof typeof DEMOSAIC_SHADERS): string {
  return `${QUANTIZE_FUNCTIONS}
// dem-method:${method}-quantize
${FUSED_PARAMS}
@group(0) @binding(0) var dem_input: texture_2d<f32>;
@group(0) @binding(1) var dem_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> params: FusedParams;
${QUANT_HELPERS}
@compute @workgroup_size(8, 8)
fn quantize_dem_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(dem_output, p, quantize_rgba(textureLoad(dem_input, p, 0), 2u, p));
}`;
}

function compilePostShader(method: keyof typeof DEMOSAIC_SHADERS): string {
  return `${QUANTIZE_FUNCTIONS}
// dem-method:${method}
${FUSED_PARAMS}
@group(0) @binding(0) var dem_input: texture_2d<f32>;
@group(0) @binding(1) var color_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var gamma_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var yuv_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> params: FusedParams;
${QUANT_HELPERS}
fn sample_dem_materialized(p: vec2<i32>) -> vec4<f32> { return textureLoad(dem_input, p, 0); }
fn sample_post_color(p: vec2<i32>) -> vec4<f32> {
  let rgb = sample_dem_materialized(p).rgb;
  let corrected = vec4<f32>(1.08 * rgb.r - 0.04 * rgb.g - 0.04 * rgb.b, -0.03 * rgb.r + 1.06 * rgb.g - 0.03 * rgb.b, -0.02 * rgb.r - 0.06 * rgb.g + 1.08 * rgb.b, 1.0);
  return quantize_rgba(corrected, 3u, p);
}
fn sample_post_gamma(p: vec2<i32>) -> vec4<f32> {
  let rgb = max(sample_post_color(p).rgb, vec3<f32>(0.0));
  return quantize_rgba(vec4<f32>(pow(rgb, vec3<f32>(1.0 / 2.2)), 1.0), 4u, p);
}
fn sample_post_yuv(p: vec2<i32>) -> vec4<f32> {
  let rgb = sample_post_gamma(p).rgb;
  let yuv = vec4<f32>(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), dot(rgb, vec3<f32>(-0.114572, -0.385428, 0.5)) + 0.5, dot(rgb, vec3<f32>(0.5, -0.454153, -0.045847)) + 0.5, 1.0);
  return quantize_rgba(yuv, 5u, p);
}
@compute @workgroup_size(8, 8)
fn postprocess_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(color_output, p, sample_post_color(p));
  textureStore(gamma_output, p, sample_post_gamma(p));
  textureStore(yuv_output, p, sample_post_yuv(p));
}`;
}

function adaptBilinearDemosaic(source: string): string {
  const withoutBindings = source.replace(/struct DemosaicParams\s*\{[\s\S]*?\}\s*;?\s*/, '').replace(/@group\(0\)[^\n]*\n/g, '');
  const adapted = replaceWgslFunction(withoutBindings, 'sample', 'fn sample_demosaic_input(p: vec2<i32>, extent: vec2<u32>) -> f32 { return sample_wbc(p); }')
    .replace(/\bsample\(/g, 'sample_demosaic_input(');
  return `${adapted.replace(/@compute[\s\S]*$/, '')}
fn sample_dem(p: vec2<i32>) -> vec4<f32> {
  let extent = source_extent(); var sums = vec3<f32>(0.0); var counts = vec3<f32>(0.0);
  let low = max(p - vec2<i32>(1), vec2<i32>(0)); let high = min(p + vec2<i32>(1), vec2<i32>(extent) - 1);
  for (var y = low.y; y <= high.y; y++) { for (var x = low.x; x <= high.x; x++) { let q = vec2<i32>(x, y); let channel = cfa(q, extent); sums[channel] += sample_demosaic_input(q, extent); counts[channel] += 1.0; } }
  return vec4<f32>(sums / max(counts, vec3<f32>(1.0)), 1.0);
}`;
}

function sanitizeDemosaicShader(source: string): string {
  return source
    .replace(/if \(dx == -2 && dy == 0 \|\| dx == 2 && dy == 0\)/g, 'if ((dx == -2 && dy == 0) || (dx == 2 && dy == 0))')
    .replace(/if \(dx == -1 && dy == 0 \|\| dx == 1 && dy == 0\)/g, 'if ((dx == -1 && dy == 0) || (dx == 1 && dy == 0))')
    .replace(/if \(dx == 0 && abs\(dy\) == 2 \|\| abs\(dx\) == 2 && dy == 0\)/g, 'if ((dx == 0 && abs(dy) == 2) || (abs(dx) == 2 && dy == 0))')
    .replace(/if \(abs\(dx\) == 1 && abs\(dy\) == 1\)/g, 'if ((abs(dx) == 1) && (abs(dy) == 1))');
}

function replaceWgslFunction(source: string, name: string, replacement: string): string {
  const start = source.indexOf(`fn ${name}(`);
  if (start < 0) throw new Error(`FUSED_SHADER_INVALID: missing ${name} function`);
  const bodyStart = source.indexOf('{', start);
  if (bodyStart < 0) throw new Error(`FUSED_SHADER_INVALID: ${name} has no body`);
  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') { depth -= 1; if (depth === 0) return `${source.slice(0, start)}${replacement}${source.slice(index + 1)}`; }
  }
  throw new Error(`FUSED_SHADER_INVALID: ${name} body is unterminated`);
}
