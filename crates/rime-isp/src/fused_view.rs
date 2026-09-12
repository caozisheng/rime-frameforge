//! The Normal Graph **fused view** — single-pass composition of the
//! post-DRC chain (WBC passthrough, demosaic, color reproduce, gamma,
//! rgb2yuv) plus the standalone pre-DRC WBC pass.
//!
//! This module is the single source of the fused shader *composition*
//! (the WebGPU worker consumes `fused_pipeline.generated.ts` verbatim;
//! the native executor dispatches per-module passes — same algorithms,
//! different runtime arrangement, per AGENTS.md "runtime only" split).
//! All algorithm bodies live in the owning modules' `.wgsl` files and are
//! `include_str!`-ed here; only the glue (param structs, binding layout,
//! port quantization helpers, adapters renaming module bindings to the
//! fused environment) lives in this file.
//!
//! # Fused uniform (`FusedParams`)
//!
//! One super-uniform shared by all fused entries; [`pack_fused_uniforms`]
//! is the Rust authority for its byte layout — the wasm derivation face
//! returns packed bytes and the web worker writes them verbatim.
use std::cmp::Ordering;
use std::str::FromStr;

/// Uniform struct shared by every fused entry.
const FUSED_PARAMS: &str = r"struct FusedParams {
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
  cr_hs_dims_and_enable: vec4<u32>,
  hr_gain_enable: vec4<f32>,
}";

/// Port-quantization helpers over `params.quant_params` (Rime.Q verification
/// layer; native runs the unquantized path).
const QUANT_HELPERS: &str = r"fn quantization_enabled(index: u32) -> bool {
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
}";

/// Color-reproduce helpers (sensor→ProPhoto, HSV HS-LUT, ProPhoto→sRGB).
/// Mirrors `color_reproduce00.wgsl` math against the fused super-uniform.
const CR_HELPERS: &str = r"fn cr_sensor_to_prophoto(row: u32, col: u32) -> f32 {
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
fn cr_hs_lut_apply(hsv: vec3<f32>) -> vec3<f32> {
  if (params.cr_hs_dims_and_enable.z == 0u) { return hsv; }
  let hue_divs = max(params.cr_hs_dims_and_enable.x, 1u);
  let sat_divs = max(params.cr_hs_dims_and_enable.y, 1u);
  let h = min(u32(floor((hsv.x % 360.0) / 360.0 * f32(hue_divs))), hue_divs - 1u);
  let s = min(u32(floor(clamp(hsv.y, 0.0, 1.0) * f32(sat_divs))), sat_divs - 1u);
  let entry = h * sat_divs + s;
  let hue_shift = cr_hs_lut.values[3u * entry + 0u];
  let sat_scale = cr_hs_lut.values[3u * entry + 1u];
  let val_scale = cr_hs_lut.values[3u * entry + 2u];
  return vec3<f32>((hsv.x + hue_shift) % 360.0, clamp(hsv.y * sat_scale, 0.0, 1.0), clamp(hsv.z * val_scale, 0.0, 1.0));
}";

/// Gamma helpers (luminance-domain Hermite LUT, per AGENTS.md hue rule).
const GAMMA_HELPERS: &str = r"fn gamma_lut_value(index: u32) -> f32 { return params.gamma_lut[index / 4u][index % 4u]; }
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
}";

/// Post-DRC chain helpers (dem quantize, CR, gamma, rgb2yuv samplers).
const POSTPROCESS_HELPERS: &str = r"fn sample_dem(p: vec2<i32>) -> vec4<f32> {
  let extent = source_extent();
  return vec4<f32>(dem00_sample(p, extent).rgb, 1.0);
}
fn sample_dem_quantized(p: vec2<i32>) -> vec4<f32> { return quantize_rgba(sample_dem(p), 2u, p); }
fn sample_color_reproduce(p: vec2<i32>) -> vec4<f32> {
  var rgb = clamp(cr_apply_sensor_to_prophoto(sample_dem_quantized(p).rgb), vec3<f32>(0.0), vec3<f32>(1.0));
  var hsv = cr_rgb_to_hsv(rgb);
  hsv = cr_hs_lut_apply(hsv);
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
}";

/// Coordinate helpers shared by fused entries.
const SOURCE_HELPERS: &str = r"fn source_extent() -> vec2<u32> { return vec2<u32>(params.width, params.height); }
fn clamp_source(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2<i32>(0), vec2<i32>(source_extent()) - vec2<i32>(1)); }
fn channel_at(p: vec2<i32>) -> u32 {
  let phase = vec2<u32>(u32(p.x) & 1u, u32(p.y) & 1u);
  return params.cfa_pattern[phase.y * 2u + phase.x];
}";

/// Identity WBC sampler for the fused post-DRC chain: WBC already ran in
/// its own pre-DRC pass, so the fused view only re-applies the s0.14 port
/// quantization when consuming the (clamped) DRC output.
const WBC_PASSTHROUGH: &str = r"fn sample_wbc(p: vec2<i32>) -> f32 {
  let q = clamp_source(p);
  return clamp(quantize_scalar(textureLoad(drc_input, q, 0).x, 1u, q), 0.0, module_saturation(1u));
}";

/// The `quantize.wgsl` prefix (`QuantParams` struct + `quantize_sample`) from
/// `rime-quant` — the single source of the quantizer.
const QUANTIZE_FULL: &str = include_str!("../../rime-quant/shaders/quantize.wgsl");

fn quantize_prefix() -> String {
    const MARKER: &str = "@group(0)";
    let index = QUANTIZE_FULL.find(MARKER).unwrap_or(QUANTIZE_FULL.len());
    QUANTIZE_FULL[..index].to_owned()
}

/// Strips module-owned declarations (structs, bindings, compute entries)
/// from a module shader, keeping constants and helper functions, so the
/// fused shell can re-host them against its own bindings. Handles
/// multi-line struct bodies via brace matching.
fn strip_to_functions(source: &str) -> String {
    let mut out = String::new();
    let mut skip_entry = false;
    let mut skip_struct = false;
    let mut depth: usize = 0;
    for line in source.lines() {
        let trimmed = line.trim_start();
        let open = line.matches('{').count();
        let close = line.matches('}').count();
        if skip_entry {
            depth = depth.saturating_add(open).saturating_sub(close);
            if depth == 0 {
                skip_entry = false;
            }
            continue;
        }
        if skip_struct {
            depth = depth.saturating_add(open).saturating_sub(close);
            if depth == 0 {
                skip_struct = false;
            }
            continue;
        }
        if trimmed.starts_with("struct ") {
            // The triggering line carries its own `{` (e.g. `struct Foo {`);
            // seed depth from it or the skip ends after the first body line
            // and the rest of the struct leaks into the fused output.
            depth = open.saturating_sub(close);
            skip_struct = depth > 0;
            continue;
        }
        if trimmed.starts_with("@group(0)") {
            continue;
        }
        if trimmed.starts_with("@compute") {
            // The entry's `{` is on the following `fn` line, not here.
            skip_entry = true;
            depth = 0;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The bilinear demosaic segment: dem00.wgsl functions (composable
/// convention — `dem00_sample(p, extent)` is the sampling entry) with the
/// module's `input_tex` aliased onto the fused `drc_input`.
fn adapt_bilinear_demosaic() -> String {
    let functions = strip_to_functions(include_str!("vbe/dem/dem00.wgsl"));
    functions.replace("input_tex", "drc_input")
}

/// Demosaic method ids whose WGSL needs the rgba32float->rgba16float and
/// operator-precedence-parenthesization sanitization pass on the WebGPU
/// worker (kept in the shared adapter below).
const DEMOSAIC_SHADER_SOURCES: &[(&str, &str)] = &[
    ("01", include_str!("vbe/dem/dem01.wgsl")),
    ("02", include_str!("vbe/dem/dem02.wgsl")),
    ("03", include_str!("vbe/dem/dem03.wgsl")),
    ("04", include_str!("vbe/dem/dem04.wgsl")),
];

/// WebGPU-worker sanitization for complex demosaic methods: storage format
/// downgrade and parenthesized precedence fixes (mirrors the previous TS
/// `sanitizeDemosaicShader` exactly).
#[must_use]
pub fn sanitize_demosaic_shader(source: &str) -> String {
    source
        .replace(
            "texture_storage_2d<rgba32float",
            "texture_storage_2d<rgba16float",
        )
        .replace(
            "if (dx == -2 && dy == 0 || dx == 2 && dy == 0)",
            "if ((dx == -2 && dy == 0) || (dx == 2 && dy == 0))",
        )
        .replace(
            "if (dx == -1 && dy == 0 || dx == 1 && dy == 0)",
            "if ((dx == -1 && dy == 0) || (dx == 1 && dy == 0))",
        )
        .replace(
            "if (dx == 0 && abs(dy) == 2 || abs(dx) == 2 && dy == 0)",
            "if ((dx == 0 && abs(dy) == 2) || (abs(dx) == 2 && dy == 0))",
        )
        .replace(
            "if (abs(dx) == 1 && abs(dy) == 1)",
            "if ((abs(dx) == 1) && (abs(dy) == 1))",
        )
}

/// Renders the segmented (complex-DEM) Normal Graph shader set: pre, dem,
/// quantize, post — same param struct and helper sources as the fused view.
// `Result` is already `#[must_use]`; a bare attribute here would be redundant.
pub fn render_segmented_normal_shaders(dem_method: &str) -> Result<[String; 4], String> {
    if dem_method == "00" {
        return Err("FUSED_GRAPH_SEGMENT_INVALID: bilinear DEM should use full fusion".to_owned());
    }
    let source = DEMOSAIC_SHADER_SOURCES
        .iter()
        .find(|(id, _)| *id == dem_method)
        .map(|(_, source)| *source)
        .ok_or_else(|| format!("FUSED_GRAPH_METHOD_INVALID: unknown DEM method {dem_method}"))?;
    let dem = sanitize_demosaic_shader(source);
    let quantize_prefix = quantize_prefix();

    let mut pre = String::new();
    pre.push_str(&quantize_prefix);
    pre.push('\n');
    pre.push_str(FUSED_PARAMS);
    pre.push_str(BINDINGS_PRE);
    pre.push_str(QUANT_HELPERS);
    pre.push_str(SOURCE_HELPERS);
    pre.push_str(WBC_PASSTHROUGH);
    pre.push_str(ENTRY_PRE);

    let quantize = quantize_prefix.clone()
        + "\n// dem-method:"
        + dem_method
        + "-quantize\n"
        + FUSED_PARAMS
        + BINDINGS_DEM_QUANTIZE
        + QUANT_HELPERS
        + ENTRY_DEM_QUANTIZE;

    let mut post = String::new();
    post.push_str(&quantize_prefix);
    post.push_str("\n// dem-method:");
    post.push_str(dem_method);
    post.push('\n');
    post.push_str(FUSED_PARAMS);
    post.push_str(BINDINGS_POST);
    post.push_str(QUANT_HELPERS);
    post.push_str(CR_HELPERS);
    post.push_str(GAMMA_HELPERS);
    post.push_str("fn sample_dem_materialized(p: vec2<i32>) -> vec4<f32> { return textureLoad(dem_input, p, 0); }\n");
    post.push_str(POSTPROCESS_HELPERS);
    post.push_str(ENTRY_POST);

    Ok([pre, dem, quantize, post])
}

const BINDINGS_PRE: &str = "@group(0) @binding(0) var drc_input: texture_2d<f32>;\n@group(0) @binding(1) var pre_output: texture_storage_2d<r32float, write>;\n@group(0) @binding(2) var<uniform> params: FusedParams;\n";
const ENTRY_PRE: &str = "@compute @workgroup_size(8, 8)\nfn pre_demosaic_main(@builtin(global_invocation_id) gid: vec3<u32>) {\n  if (gid.x >= params.width || gid.y >= params.height) { return; }\n  let p = vec2<i32>(gid.xy);\n  textureStore(pre_output, p, vec4<f32>(sample_wbc(p), 0.0, 0.0, 1.0));\n}\n";
const BINDINGS_DEM_QUANTIZE: &str = "@group(0) @binding(0) var dem_input: texture_2d<f32>;\n@group(0) @binding(1) var dem_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(2) var<uniform> params: FusedParams;\n";
const ENTRY_DEM_QUANTIZE: &str = "@compute @workgroup_size(8, 8)\nfn quantize_dem_main(@builtin(global_invocation_id) gid: vec3<u32>) {\n  if (gid.x >= params.width || gid.y >= params.height) { return; }\n  let p = vec2<i32>(gid.xy);\n  textureStore(dem_output, p, quantize_rgba(textureLoad(dem_input, p, 0), 2u, p));\n}\n";
const BINDINGS_POST: &str = "@group(0) @binding(0) var dem_input: texture_2d<f32>;\n@group(0) @binding(1) var color_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(2) var gamma_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(3) var yuv_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(4) var<uniform> params: FusedParams;\nstruct FloatBuffer { values: array<f32> }\n@group(0) @binding(5) var<storage, read> cr_hs_lut: FloatBuffer;\n";
const ENTRY_POST: &str = "@compute @workgroup_size(8, 8)\nfn postprocess_main(@builtin(global_invocation_id) gid: vec3<u32>) {\n  if (gid.x >= params.width || gid.y >= params.height) { return; }\n  let p = vec2<i32>(gid.xy);\n  textureStore(color_output, p, sample_color_reproduce(p));\n  textureStore(gamma_output, p, sample_gamma(p));\n  textureStore(yuv_output, p, sample_rgb2yuv(p));\n}\n";

/// Renders the complete fused Normal Graph shader (post-DRC single pass).
#[must_use]
pub fn render_fused_normal_shader() -> String {
    let quantize_prefix = quantize_prefix();
    format!(
        "{quantize_prefix}\n// dem-method:00\n{FUSED_PARAMS}\n@group(0) @binding(0) var drc_input: texture_2d<f32>;\n@group(0) @binding(1) var wbc_output: texture_storage_2d<r32float, write>;\n@group(0) @binding(2) var dem_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(3) var color_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(4) var gamma_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(5) var yuv_output: texture_storage_2d<rgba16float, write>;\n@group(0) @binding(6) var<uniform> params: FusedParams;\nstruct FloatBuffer {{ values: array<f32> }}\n@group(0) @binding(7) var<storage, read> cr_hs_lut: FloatBuffer;\n{QUANT_HELPERS}\n{CR_HELPERS}\n{GAMMA_HELPERS}\n{SOURCE_HELPERS}\n{WBC_PASSTHROUGH}\n{}\n{POSTPROCESS_HELPERS}\n@compute @workgroup_size(8, 8)\nfn normal_fused_main(@builtin(global_invocation_id) gid: vec3<u32>) {{\n  if (gid.x >= params.width || gid.y >= params.height) {{ return; }}\n  let p = vec2<i32>(gid.xy);\n  textureStore(wbc_output, p, vec4<f32>(sample_wbc(p), 0.0, 0.0, 1.0));\n  textureStore(dem_output, p, sample_dem_quantized(p));\n  textureStore(color_output, p, sample_color_reproduce(p));\n  textureStore(gamma_output, p, sample_gamma(p));\n  textureStore(yuv_output, p, sample_rgb2yuv(p));\n}}",
        adapt_bilinear_demosaic()
    )
}

/// Quantized module output ports carried in the fused super-uniform, in
/// `FusedParams.quant_params` order.
pub const FUSED_QUANT_MODULE_IDS: [&str; 6] =
    ["blc", "wbc", "dem", "color_reproduce", "gamma", "rgb2yuv"];

/// Total byte size of the packed `FusedParams` uniform block.
pub const FUSED_UNIFORM_BYTES: usize = 672;

/// Header layout: `width..frame_index` (16 words before `quant_params`).
const HEADER_WORDS: usize = 16;
/// `quant_params` stride: one `QuantParams` block (16 words).
const QUANT_BLOCK_WORDS: usize = 16;
/// `quant_enabled_0` word offset in the packed block.
const QUANT_ENABLED_OFFSET: usize = 448;
/// `gamma_and_padding.x` word offset (gamma value; `.yzw` are padding).
const GAMMA_OFFSET: usize = 480;
/// `gamma_lut` word offset: nine f32 knots packed into three `vec4<f32>`.
const GAMMA_LUT_OFFSET: usize = 496;
/// `cr_sensor_to_prophoto` word offset (3×3 row-major, `vec4`-strided).
const CR_SENSOR_TO_PROPHOTO_OFFSET: usize = 544;
/// `cr_prophoto_to_srgb` word offset.
const CR_PROPHOTO_TO_SRGB_OFFSET: usize = 592;
/// `cr_hs_dims_and_enable` word offset (x=h dims, y=v dims, z=enable).
const CR_HS_DIMS_OFFSET: usize = 640;
/// `hr_gain_enable` word offset (x = `hr_gain`, y = enable flag).
const HR_GAIN_ENABLE_OFFSET: usize = 656;

/// Color-reproduce assets frozen at descriptor build time.
#[derive(Clone, Copy, Debug)]
pub struct FusedColorReproduce {
    /// Row-major 3×3 sensor→ProPhoto matrix.
    pub sensor_to_prophoto: [f32; 9],
    /// Row-major 3×3 ProPhoto→sRGB matrix.
    pub prophoto_to_srgb: [f32; 9],
    /// HS-LUT grid dimensions `[h, v]`.
    pub hs_dims: [u32; 2],
    /// Whether the HSV HS-LUT path is enabled.
    pub hs_enable: bool,
}

/// Demosaic thresholds mirrored into the fused header.
#[derive(Clone, Copy, Debug)]
pub struct FusedDemosaicThresholds {
    /// VNG gradient threshold.
    pub vng_threshold: f32,
    /// AHD luminance threshold.
    pub ahd_l_threshold: f32,
    /// AHD chroma threshold squared.
    pub ahd_c_threshold_sq: f32,
}

/// Gamma block values (exponent + nine-knot luminance LUT).
#[derive(Clone, Debug)]
pub struct FusedGamma {
    /// Display exponent (`gamma_and_padding.x`).
    pub gamma: f32,
    /// Nine monotone luminance LUT knots with fixed endpoints 0 and 1.
    pub lut: [f32; 9],
}

/// Highlight-recovery block values.
#[derive(Clone, Copy, Debug)]
pub struct FusedHighlightRecovery {
    /// Whether HR runs this frame (e.g. gains render to `hr_gain=1`).
    pub enable: bool,
    /// Green-normalized white-balance gains `[r, g, b]`.
    pub gains: [f32; 3],
}

/// Everything [`pack_fused_uniforms`] needs, grouped to keep the call site
/// declarative; mirrors the web `packFusedUniforms` inputs one-for-one.
#[derive(Clone, Debug)]
pub struct FusedUniformRequest {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Sensor black level (raw code units).
    pub black_level: f32,
    /// Sensor white level (raw code units).
    pub white_level: f32,
    /// CFA channel per phase position (row-major 2×2).
    pub cfa_pattern: [u32; 4],
    /// Green-normalized white-balance gains `[r, g, b]`.
    pub white_balance_gains: [f32; 3],
    /// Demosaic thresholds.
    pub demosaic: FusedDemosaicThresholds,
    /// Gamma block.
    pub gamma: FusedGamma,
    /// Color-reproduce assets.
    pub color_reproduce: FusedColorReproduce,
    /// Highlight-recovery block.
    pub highlight_recovery: FusedHighlightRecovery,
    /// Frame index (dither stream input).
    pub frame_index: u32,
    /// Per-module quantization preferences (order from the graph
    /// quantization config; stream ids derive from position).
    pub quantization: Vec<rime_quant::GpuQuantModuleConfig>,
    /// Whether the whole graph quantization layer is enabled.
    pub quantization_graph_enabled: bool,
    /// Module execution modes from the graph presentation (quantization
    /// only applies to enabled modules).
    pub module_modes: Vec<(String, bool)>,
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_matrix3(bytes: &mut [u8], base: usize, values: &[f32; 9]) {
    for row in 0..3 {
        for col in 0..3 {
            write_f32(bytes, base + (row * 4 + col) * 4, values[row * 3 + col]);
        }
    }
}

/// Packs the fused super-uniform (`FusedParams`) — the Rust authority for
/// its byte layout.
///
/// # Errors
///
/// Returns a stable `FUSED_UNIFORM_*` prefixed message when the gamma
/// block, quantization preferences, or white-balance gains are invalid.
///
/// # Panics
///
/// Never panics.
pub fn pack_fused_uniforms(request: &FusedUniformRequest) -> Result<Vec<u8>, String> {
    if !(1.8..=2.4).contains(&request.gamma.gamma) {
        return Err("FUSED_UNIFORM_GAMMA_INVALID: gamma must be within 1.8..=2.4".into());
    }
    if request.gamma.lut[0] != 0.0 || (request.gamma.lut[8] - 1.0).abs() > f32::EPSILON {
        return Err("FUSED_UNIFORM_GAMMA_LUT_INVALID: endpoints must be 0 and 1".into());
    }
    if !request.gamma.lut.windows(2).all(|pair| {
        matches!(
            pair[0].partial_cmp(&pair[1]),
            Some(Ordering::Less | Ordering::Equal)
        ) && pair[0].is_finite()
            && pair[1].is_finite()
    }) {
        return Err("FUSED_UNIFORM_GAMMA_LUT_INVALID: must be monotone and finite".into());
    }
    if !request
        .white_balance_gains
        .iter()
        .all(|gain| gain.is_finite() && *gain > 0.0)
    {
        return Err("FUSED_UNIFORM_WBC_GAINS_INVALID: gains must be finite and positive".into());
    }

    let mut bytes = vec![0_u8; FUSED_UNIFORM_BYTES];
    write_u32(&mut bytes, 0, request.width);
    write_u32(&mut bytes, 4, request.height);
    write_f32(&mut bytes, 8, request.black_level);
    write_f32(&mut bytes, 12, request.white_level);
    for (index, channel) in request.cfa_pattern.iter().enumerate() {
        write_u32(&mut bytes, 16 + index * 4, *channel);
    }
    for (index, gain) in request.white_balance_gains.iter().enumerate() {
        write_f32(&mut bytes, 32 + index * 4, *gain);
    }
    write_f32(&mut bytes, 48, request.demosaic.vng_threshold);
    write_f32(&mut bytes, 52, request.demosaic.ahd_l_threshold);
    write_f32(&mut bytes, 56, request.demosaic.ahd_c_threshold_sq);
    write_u32(&mut bytes, 60, request.frame_index);

    write_quantization_blocks(&mut bytes, request)?;

    write_f32(&mut bytes, GAMMA_OFFSET, request.gamma.gamma);
    for (index, value) in request.gamma.lut.iter().enumerate() {
        write_f32(&mut bytes, GAMMA_LUT_OFFSET + index * 4, *value);
    }
    write_matrix3(
        &mut bytes,
        CR_SENSOR_TO_PROPHOTO_OFFSET,
        &request.color_reproduce.sensor_to_prophoto,
    );
    write_matrix3(
        &mut bytes,
        CR_PROPHOTO_TO_SRGB_OFFSET,
        &request.color_reproduce.prophoto_to_srgb,
    );
    write_u32(
        &mut bytes,
        CR_HS_DIMS_OFFSET,
        request.color_reproduce.hs_dims[0],
    );
    write_u32(
        &mut bytes,
        CR_HS_DIMS_OFFSET + 4,
        request.color_reproduce.hs_dims[1],
    );
    write_u32(
        &mut bytes,
        CR_HS_DIMS_OFFSET + 8,
        u32::from(request.color_reproduce.hs_enable),
    );
    let gains = crate::vfe::white_balance::WhiteBalanceGains {
        red: request.highlight_recovery.gains[0],
        green: request.highlight_recovery.gains[1],
        blue: request.highlight_recovery.gains[2],
    };
    let (hr_gain, hr_enable) = if request.highlight_recovery.enable {
        let gain = crate::vfe::white_balance::highlight_recovery_gain(&gains)
            .map_err(|error| format!("FUSED_UNIFORM_HR_GAIN_INVALID: {error}"))?;
        (gain, 1.0_f32)
    } else {
        (1.0, 0.0)
    };
    write_f32(&mut bytes, HR_GAIN_ENABLE_OFFSET, hr_gain);
    write_f32(&mut bytes, HR_GAIN_ENABLE_OFFSET + 4, hr_enable);
    Ok(bytes)
}

/// Packs the six per-module quantization blocks and their enable words.
fn write_quantization_blocks(
    bytes: &mut [u8],
    request: &FusedUniformRequest,
) -> Result<(), String> {
    for (index, module_id) in FUSED_QUANT_MODULE_IDS.iter().enumerate() {
        let Some(config) = request
            .quantization
            .iter()
            .find(|config| config.module_id == *module_id)
        else {
            return Err(format!(
                "FUSED_UNIFORM_QUANT_MISSING: {module_id} preference absent"
            ));
        };
        let stream_id = u32::try_from(index)
            .map_err(|_| "FUSED_UNIFORM_QUANT_INVALID: stream id overflow".to_string())?
            + 1;
        let module_enabled = request
            .module_modes
            .iter()
            .find(|(id, _)| id == *module_id)
            .is_some_and(|(_, enabled)| *enabled);
        let output_enabled =
            request.quantization_graph_enabled && config.output_enabled && module_enabled;
        let profile = rime_quant::RimeQProfile::from_str(config.output_profile.as_str())
            .map_err(|error| format!("FUSED_UNIFORM_QUANT_PROFILE_INVALID: {error}"))?;
        let plan = rime_quant::GpuQuantPlan::derive(rime_quant::GpuQuantPlanRequest {
            profile,
            clip_type: config.clip_type,
            output_enabled,
            stream_id,
            frame_index: request.frame_index,
            plane: 0,
            width: request.width,
            height: request.height,
        })
        .map_err(|error| format!("FUSED_UNIFORM_QUANT_PLAN_INVALID: {error}"))?;
        let base = (HEADER_WORDS + index * QUANT_BLOCK_WORDS) * 4;
        bytes[base..base + rime_quant::QUANT_PARAMS_BYTES].copy_from_slice(&plan.to_wgsl_bytes());
        if plan.output_enabled {
            write_u32(bytes, QUANT_ENABLED_OFFSET + index * 4, 1);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> FusedUniformRequest {
        FusedUniformRequest {
            width: 3744,
            height: 2776,
            black_level: 511.0,
            white_level: 8000.0,
            cfa_pattern: [0, 1, 1, 2],
            white_balance_gains: [2.804_687_5, 1.0, 1.742_187_5],
            demosaic: FusedDemosaicThresholds {
                vng_threshold: 1.5,
                ahd_l_threshold: 2.0,
                ahd_c_threshold_sq: 4.0,
            },
            gamma: FusedGamma {
                gamma: 2.2,
                lut: [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0],
            },
            color_reproduce: FusedColorReproduce {
                sensor_to_prophoto: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                prophoto_to_srgb: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                hs_dims: [1, 1],
                hs_enable: false,
            },
            highlight_recovery: FusedHighlightRecovery {
                enable: true,
                gains: [2.804_687_5, 1.0, 1.742_187_5],
            },
            frame_index: 7,
            quantization: FUSED_QUANT_MODULE_IDS
                .map(|module_id| rime_quant::GpuQuantModuleConfig {
                    module_id: module_id.into(),
                    output_enabled: true,
                    output_profile: "s0.14".into(),
                    clip_type: rime_quant::ClipType::Truncate,
                })
                .into_iter()
                .collect(),
            quantization_graph_enabled: true,
            module_modes: FUSED_QUANT_MODULE_IDS
                .map(|id| (id.to_owned(), true))
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn packs_header_and_quant_layout() {
        let request = sample_request();
        let bytes = pack_fused_uniforms(&request).expect("valid request");
        assert_eq!(bytes.len(), FUSED_UNIFORM_BYTES);
        assert_eq!(u32_at(&bytes, 0), 3744);
        assert_eq!(u32_at(&bytes, 1), 2776);
        assert!((f32_at(&bytes, 2) - 511.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 3) - 8000.0).abs() < f32::EPSILON);
        assert_eq!(u32_at(&bytes, 4), 0);
        assert_eq!(u32_at(&bytes, 7), 2);
        assert!((f32_at(&bytes, 8) - 2.804_687_5).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 10) - 1.742_187_5).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 12) - 1.5).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 13) - 2.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 14) - 4.0).abs() < f32::EPSILON);
        assert_eq!(u32_at(&bytes, 15), 7);
        // First quant block (blc) at word 16.
        assert!((f32_at(&bytes, 16) - 16_384.0).abs() < 1.0);
        assert_eq!(u32_at(&bytes, 19), 0);
        assert_eq!(u32_at(&bytes, 112), 1);
    }

    #[test]
    fn packs_gamma_cr_and_hr_tail() {
        let request = sample_request();
        let bytes = pack_fused_uniforms(&request).expect("valid request");
        // gamma at word 120, lut at 124..133.
        assert!((f32_at(&bytes, 120) - 2.2).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 124) - 0.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 132) - 1.0).abs() < f32::EPSILON);
        // CR identity matrices at 136/148.
        assert!((f32_at(&bytes, 136) - 1.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 141) - 1.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 146) - 1.0).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 148) - 1.0).abs() < f32::EPSILON);
        // hs dims at 160, HR block at 164.
        assert_eq!(u32_at(&bytes, 160), 1);
        assert_eq!(u32_at(&bytes, 161), 1);
        assert_eq!(u32_at(&bytes, 162), 0);
        // hr_gain: median(2.8046875, 1.0, 1.7421875) = 1.7421875.
        assert!((f32_at(&bytes, 164) - 1.742_187_5).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 165) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_invalid_gamma_and_gains() {
        let mut request = sample_request();
        request.gamma.gamma = 3.0;
        assert!(
            pack_fused_uniforms(&request)
                .unwrap_err()
                .starts_with("FUSED_UNIFORM_GAMMA_INVALID")
        );
        request.gamma.gamma = 2.2;
        request.gamma.lut[4] = 0.1;
        assert!(
            pack_fused_uniforms(&request)
                .unwrap_err()
                .starts_with("FUSED_UNIFORM_GAMMA_LUT_INVALID")
        );
        request.gamma.lut[4] = 0.5;
        request.white_balance_gains = [0.0, 1.0, 1.0];
        assert!(
            pack_fused_uniforms(&request)
                .unwrap_err()
                .starts_with("FUSED_UNIFORM_WBC_GAINS_INVALID")
        );
    }

    fn f32_at(bytes: &[u8], word: usize) -> f32 {
        f32::from_ne_bytes(
            bytes[word * 4..word * 4 + 4]
                .try_into()
                .expect("4-byte slice"),
        )
    }

    fn u32_at(bytes: &[u8], word: usize) -> u32 {
        u32::from_ne_bytes(
            bytes[word * 4..word * 4 + 4]
                .try_into()
                .expect("4-byte slice"),
        )
    }
}
