import { buildFusedGraphPlan } from './fused-graph-plan.js';
import quantizeShader from '../../../crates/rime-quant/shaders/quantize.wgsl?raw';
import demBilinearShader from '../../../crates/rime-isp/src/vbe/dem/dem00.wgsl?raw';
import demMhcShader from '../../../crates/rime-isp/src/vbe/dem/dem01.wgsl?raw';
import demPpgShader from '../../../crates/rime-isp/src/vbe/dem/dem02.wgsl?raw';
import demVngShader from '../../../crates/rime-isp/src/vbe/dem/dem03.wgsl?raw';
import demAhdShader from '../../../crates/rime-isp/src/vbe/dem/dem04.wgsl?raw';

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
  gamma_and_padding: vec4<f32>,
  gamma_lut: array<vec4<f32>, 3>,
  cr_sensor_to_prophoto: array<vec4<f32>, 3>,
  cr_prophoto_to_srgb: array<vec4<f32>, 3>,
  cr_hsv_dims_and_enable: vec4<u32>,
}`;
const QUANT_HELPERS = `fn quantization_enabled(index: u32) -> bool {
  if (index < 4u) { return params.quant_enabled_0[index] != 0u; }
  return params.quant_enabled_1[index - 4u] != 0u;
}
fn module_saturation(index: u32) -> f32 {
  return select(1.0, params.quant_params[index].qmax, quantization_enabled(index));
}
fn quantize_scalar(value: f32, index: u32, p: vec2<i32>) -> f32 {
  if (!quantization_enabled(index)) { return clamp(value, 0.0, module_saturation(index)); }
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

const CR_HELPERS = `
fn cr_sensor_to_prophoto(row: u32, col: u32) -> f32 {
  if (col == 0u) { return params.cr_sensor_to_prophoto[row].x; }
  if (col == 1u) { return params.cr_sensor_to_prophoto[row].y; }
  return params.cr_sensor_to_prophoto[row].z;
}
fn cr_prophoto_to_srgb(row: u32, col: u32) -> f32 {
  if (col == 0u) { return params.cr_prophoto_to_srgb[row].x; }
  if (col == 1u) { return params.cr_prophoto_to_srgb[row].y; }
  return params.cr_prophoto_to_srgb[row].z;
}
fn cr_apply_sensor_to_prophoto(rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    cr_sensor_to_prophoto(0u, 0u) * rgb.r + cr_sensor_to_prophoto(0u, 1u) * rgb.g + cr_sensor_to_prophoto(0u, 2u) * rgb.b,
    cr_sensor_to_prophoto(1u, 0u) * rgb.r + cr_sensor_to_prophoto(1u, 1u) * rgb.g + cr_sensor_to_prophoto(1u, 2u) * rgb.b,
    cr_sensor_to_prophoto(2u, 0u) * rgb.r + cr_sensor_to_prophoto(2u, 1u) * rgb.g + cr_sensor_to_prophoto(2u, 2u) * rgb.b);
}
fn cr_apply_prophoto_to_srgb(rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    cr_prophoto_to_srgb(0u, 0u) * rgb.r + cr_prophoto_to_srgb(0u, 1u) * rgb.g + cr_prophoto_to_srgb(0u, 2u) * rgb.b,
    cr_prophoto_to_srgb(1u, 0u) * rgb.r + cr_prophoto_to_srgb(1u, 1u) * rgb.g + cr_prophoto_to_srgb(1u, 2u) * rgb.b,
    cr_prophoto_to_srgb(2u, 0u) * rgb.r + cr_prophoto_to_srgb(2u, 1u) * rgb.g + cr_prophoto_to_srgb(2u, 2u) * rgb.b);
}
fn cr_rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
  let v = max(max(rgb.r, rgb.g), rgb.b);
  let c = v - min(min(rgb.r, rgb.g), rgb.b);
  var s = 0.0;
  if (v != 0.0) { s = c / v; }
  var h = 0.0;
  if (c != 0.0) {
    if (v == rgb.r) { h = 60.0 * (((rgb.g - rgb.b) / c) % 6.0); }
    else if (v == rgb.g) { h = 60.0 * ((rgb.b - rgb.r) / c + 2.0); }
    else { h = 60.0 * ((rgb.r - rgb.g) / c + 4.0); }
  }
  return vec3<f32>(h % 360.0, clamp(s, 0.0, 1.0), clamp(v, 0.0, 1.0));
}
fn cr_hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
  let h = hsv.x % 360.0;
  let s = clamp(hsv.y, 0.0, 1.0);
  let v = clamp(hsv.z, 0.0, 1.0);
  let c = v * s;
  let x = c * (1.0 - abs((h / 60.0) % 2.0 - 1.0));
  var sector = vec3<f32>(0.0);
  let t = h / 60.0;
  if (t < 1.0) { sector = vec3<f32>(c, x, 0.0); }
  else if (t < 2.0) { sector = vec3<f32>(x, c, 0.0); }
  else if (t < 3.0) { sector = vec3<f32>(0.0, c, x); }
  else if (t < 4.0) { sector = vec3<f32>(0.0, x, c); }
  else if (t < 5.0) { sector = vec3<f32>(x, 0.0, c); }
  else { sector = vec3<f32>(c, 0.0, x); }
  return clamp(sector + vec3<f32>(v - c), vec3<f32>(0.0), vec3<f32>(1.0));
}
fn cr_hsv_lut_apply(hsv: vec3<f32>) -> vec3<f32> {
  if (params.cr_hsv_dims_and_enable.w == 0u) { return hsv; }
  let hue_divs = max(params.cr_hsv_dims_and_enable.x, 1u);
  let sat_divs = max(params.cr_hsv_dims_and_enable.y, 1u);
  let val_divs = max(params.cr_hsv_dims_and_enable.z, 1u);
  let h = min(u32(floor((hsv.x % 360.0) / 360.0 * f32(hue_divs))), hue_divs - 1u);
  let s = min(u32(floor(clamp(hsv.y, 0.0, 1.0) * f32(sat_divs))), sat_divs - 1u);
  let v = min(u32(floor(clamp(hsv.z, 0.0, 1.0) * f32(val_divs))), val_divs - 1u);
  let entry = (v * hue_divs + h) * sat_divs + s;
  let hue_shift = cr_hsv_lut.values[3u * entry + 0u];
  let sat_scale = cr_hsv_lut.values[3u * entry + 1u];
  let val_scale = cr_hsv_lut.values[3u * entry + 2u];
  return vec3<f32>((hsv.x + hue_shift) % 360.0, clamp(hsv.y * sat_scale, 0.0, 1.0), clamp(hsv.z * val_scale, 0.0, 1.0));
}
`;
export const CR_SHADER_HELPERS = CR_HELPERS;
const GAMMA_HELPERS = `fn gamma_lut_value(index: u32) -> f32 { return params.gamma_lut[index / 4u][index % 4u]; }
fn gamma_lut_secant(index: u32) -> f32 { return gamma_lut_value(index + 1u) - gamma_lut_value(index); }
fn gamma_lut_tangent(index: u32) -> f32 {
  if (index == 0u) { return gamma_lut_secant(0u); }
  if (index >= 8u) { return gamma_lut_secant(7u); }
  let left = gamma_lut_secant(index - 1u); let right = gamma_lut_secant(index);
  if (left * right <= 0.0) { return 0.0; }
  return 2.0 * left * right / (left + right);
}
fn sample_gamma_luminance_lut(value: f32) -> f32 {
  if (value > 1.0) { return value; }
  let coordinate = clamp(value, 0.0, 1.0) * 8.0;
  let index = min(u32(floor(coordinate)), 7u); let t = coordinate - f32(index);
  let y0 = gamma_lut_value(index); let y1 = gamma_lut_value(index + 1u);
  let control1 = y0 + gamma_lut_tangent(index) / 3.0; let control2 = y1 - gamma_lut_tangent(index + 1u) / 3.0;
  let one_minus_t = 1.0 - t;
  let mapped = one_minus_t * one_minus_t * one_minus_t * y0 + 3.0 * one_minus_t * one_minus_t * t * control1 + 3.0 * one_minus_t * t * t * control2 + t * t * t * y1;
  return clamp(mapped, min(y0, y1), max(y0, y1));
}
fn apply_gamma_luminance_lut(rgb_input: vec3<f32>) -> vec3<f32> {
  let rgb = max(rgb_input, vec3<f32>(0.0));
  let luminance = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
  var mapped_rgb = vec3<f32>(0.0);
  if (luminance > 0.000001) { mapped_rgb = rgb * (sample_gamma_luminance_lut(luminance) / luminance); }
  return pow(max(mapped_rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / max(params.gamma_and_padding.x, 0.000001)));
}`;

export interface SegmentedNormalShaders {
  readonly pre: string;
  readonly dem: string;
  readonly quantize: string;
  readonly post: string;
}

export function compileBlcShader(): string {
  return `${QUANTIZE_FUNCTIONS}
${FUSED_PARAMS}
@group(0) @binding(0) var raw_input: texture_2d<u32>;
@group(0) @binding(1) var blc_output: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: FusedParams;
${QUANT_HELPERS}
${rawBlcFunctions()}
@compute @workgroup_size(8, 8)
fn blc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(blc_output, p, vec4<f32>(sample_blc(p), 0.0, 0.0, 1.0));
}`;
}

export function compileFusedNormalShader(demMethod: keyof typeof DEMOSAIC_SHADERS = '00'): string {
  const plan = buildFusedGraphPlan();
  if (plan.nodes.length !== 7 || plan.previewNodeId !== 'rgb2yuv') throw new Error('FUSED_GRAPH_INVALID: unexpected Normal Graph plan');
  if (demMethod !== '00') throw new Error(`FUSED_GRAPH_BOUNDARY: DEM method ${demMethod} requires a materialization boundary`);
  const demosaic = adaptBilinearDemosaic(demBilinearShader);
  return `${QUANTIZE_FUNCTIONS}
// dem-method:00
${FUSED_PARAMS}
@group(0) @binding(0) var drc_input: texture_2d<f32>;
@group(0) @binding(1) var wbc_output: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var dem_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var color_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var gamma_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var yuv_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var<uniform> params: FusedParams;
struct FloatBuffer { values: array<f32> }
@group(0) @binding(7) var<storage, read> cr_hsv_lut: FloatBuffer;
${QUANT_HELPERS}
${CR_HELPERS}
${GAMMA_HELPERS}
${drcWbcFunctions()}
${demosaic}
${postprocessFunctions('sample_dem(p)')}
@compute @workgroup_size(8, 8)
fn normal_fused_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(wbc_output, p, vec4<f32>(sample_wbc(p), 0.0, 0.0, 1.0));
  textureStore(dem_output, p, sample_dem_quantized(p));
  textureStore(color_output, p, sample_color_reproduce(p));
  textureStore(gamma_output, p, sample_gamma(p));
  textureStore(yuv_output, p, sample_rgb2yuv(p));
}`;
}

export function compileSegmentedNormalShaders(demMethod: keyof typeof DEMOSAIC_SHADERS): SegmentedNormalShaders {
  const plan = buildFusedGraphPlan();
  if (plan.nodes.length !== 7 || plan.previewNodeId !== 'rgb2yuv') throw new Error('FUSED_GRAPH_INVALID: unexpected Normal Graph plan');
  if (demMethod === '00') throw new Error('FUSED_GRAPH_SEGMENT_INVALID: bilinear DEM should use full fusion');
  return {
    pre: compilePreShader(),
    dem: sanitizeDemosaicShader(DEMOSAIC_SHADERS[demMethod]),
    quantize: compileDemQuantizeShader(demMethod),
    post: compilePostShader(demMethod),
  };
}

function rawBlcFunctions(): string {
  return `fn source_extent() -> vec2<u32> { return vec2<u32>(params.width, params.height); }
fn clamp_source(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2<i32>(0), vec2<i32>(source_extent()) - vec2<i32>(1)); }
fn sample_raw(p: vec2<i32>) -> f32 { return f32(textureLoad(raw_input, clamp_source(p), 0).r); }
fn sample_blc(p: vec2<i32>) -> f32 {
  let q = clamp_source(p);
  return clamp(quantize_scalar((sample_raw(q) - params.black_level) / (params.white_level - params.black_level), 0u, q), 0.0, module_saturation(0u));
}`;
}

function drcWbcFunctions(): string {
  return `fn source_extent() -> vec2<u32> { return vec2<u32>(params.width, params.height); }
fn clamp_source(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2<i32>(0), vec2<i32>(source_extent()) - vec2<i32>(1)); }
fn sample_wbc(p: vec2<i32>) -> f32 {
  let q = clamp_source(p);
  let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u);
  let channel = params.cfa_pattern[phase.y * 2u + phase.x];
  let gain = params.white_balance_gains[channel];
  return clamp(quantize_scalar(textureLoad(drc_input, q, 0).x * gain, 1u, q), 0.0, module_saturation(1u));
}`;
}

function postprocessFunctions(demExpression: string): string {
  return `fn sample_dem_quantized(p: vec2<i32>) -> vec4<f32> { return quantize_rgba(${demExpression}, 2u, p); }
fn sample_color_reproduce(p: vec2<i32>) -> vec4<f32> {
  // Real color reproduce: sensor->ProPhoto matrix, HSV LUT calibration,
  // ProPhoto->sRGB matrix, three per-channel clips (MATLAB steps 7-12).
  // Matrices and LUT come from the Rust preprocess via descriptor assets.
  var rgb = clamp(cr_apply_sensor_to_prophoto(sample_dem_quantized(p).rgb), vec3<f32>(0.0), vec3<f32>(1.0));
  var hsv = cr_rgb_to_hsv(rgb);
  hsv = cr_hsv_lut_apply(hsv);
  rgb = clamp(cr_hsv_to_rgb(hsv), vec3<f32>(0.0), vec3<f32>(1.0));
  let srgb = clamp(cr_apply_prophoto_to_srgb(rgb), vec3<f32>(0.0), vec3<f32>(1.0));
  return quantize_rgba(vec4<f32>(srgb, 1.0), 3u, p);
}
fn sample_gamma(p: vec2<i32>) -> vec4<f32> {
  let encoded = apply_gamma_luminance_lut(sample_color_reproduce(p).rgb);
  return quantize_rgba(vec4<f32>(encoded, 1.0), 4u, p);
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
@group(0) @binding(0) var drc_input: texture_2d<f32>;
@group(0) @binding(1) var pre_output: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: FusedParams;
${QUANT_HELPERS}
${drcWbcFunctions()}
@compute @workgroup_size(8, 8)
fn pre_demosaic_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let p = vec2<i32>(gid.xy);
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
struct FloatBuffer { values: array<f32> }
@group(0) @binding(5) var<storage, read> cr_hsv_lut: FloatBuffer;
${QUANT_HELPERS}
${CR_HELPERS}
${GAMMA_HELPERS}
fn sample_dem_materialized(p: vec2<i32>) -> vec4<f32> { return textureLoad(dem_input, p, 0); }
fn sample_post_color(p: vec2<i32>) -> vec4<f32> {
  var rgb = clamp(cr_apply_sensor_to_prophoto(sample_dem_materialized(p).rgb), vec3<f32>(0.0), vec3<f32>(1.0));
  var hsv = cr_rgb_to_hsv(rgb);
  hsv = cr_hsv_lut_apply(hsv);
  rgb = clamp(cr_hsv_to_rgb(hsv), vec3<f32>(0.0), vec3<f32>(1.0));
  let srgb = clamp(cr_apply_prophoto_to_srgb(rgb), vec3<f32>(0.0), vec3<f32>(1.0));
  return quantize_rgba(vec4<f32>(srgb, 1.0), 3u, p);
}
fn sample_post_gamma(p: vec2<i32>) -> vec4<f32> {
  let encoded = apply_gamma_luminance_lut(sample_post_color(p).rgb);
  return quantize_rgba(vec4<f32>(encoded, 1.0), 4u, p);
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
  return vec4<f32>(shared_saturation_clip(sums / max(counts, vec3<f32>(1.0))), 1.0);
}`;
}

function sanitizeDemosaicShader(source: string): string {
  return source
    .replace(/texture_storage_2d<rgba32float/g, 'texture_storage_2d<rgba16float')
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
