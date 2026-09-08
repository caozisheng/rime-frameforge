const GRADIENT_GUIDED_RADIUS_1: i32 = 1;
const GRADIENT_GUIDED_RADIUS_3: i32 = 3;
const GRADIENT_GUIDED_EPS_DYN: f32 = 1e-6;

struct DrcParams {
  drc_gain: f32,
  knee: f32,
  amplifier: f32,
  luma_guard: f32,
  min_ratio: f32,
  max_ratio: f32,
  level_count: u32,
  feature_flags: u32,
}
struct FloatBuffer { values: array<f32> }

@group(0) @binding(0) var<uniform> params: DrcParams;
@group(0) @binding(1) var input_a: texture_2d<f32>;
@group(0) @binding(2) var input_b: texture_2d<f32>;
@group(0) @binding(3) var input_c: texture_2d<f32>;
@group(0) @binding(4) var output_r32: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var output_rgba: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var<storage, read> global_lut: FloatBuffer;
@group(0) @binding(7) var<storage, read> local_lut: FloatBuffer;
@group(0) @binding(8) var<storage, read> modulation_luts: FloatBuffer;


fn load_zero(texture: texture_2d<f32>, position: vec2<i32>) -> vec4<f32> {
  let size = textureDimensions(texture);
  if (position.x < 0 || position.y < 0 || position.x >= i32(size.x) || position.y >= i32(size.y)) {
    return vec4<f32>(0.0);
  }
  return textureLoad(texture, position, 0);
}


fn cubic(value_in: f32) -> f32 {
  let value = abs(value_in);
  if (value <= 1.0) { return (1.5 * value - 2.5) * value * value + 1.0; }
  if (value < 2.0) { return ((-0.5 * value + 2.5) * value - 4.0) * value + 2.0; }
  return 0.0;
}

fn mirror(value_in: i32, length: i32) -> i32 {
  if (length <= 1) { return 0; }
  let period = 2 * length;
  var value = value_in % period;
  if (value < 0) { value += period; }
  if (value < length) { return value; }
  return period - value - 1;
}

fn resize_sample(texture: texture_2d<f32>, pixel: vec2<u32>, output_size: vec2<u32>) -> f32 {
  let input_size = textureDimensions(texture);
  let source = (vec2<f32>(pixel) + vec2<f32>(0.5)) * vec2<f32>(input_size) / vec2<f32>(output_size) - vec2<f32>(0.5);
  let base = vec2<i32>(floor(source));
  let scale = min(vec2<f32>(output_size) / vec2<f32>(input_size), vec2<f32>(1.0));
  var sum = 0.0;
  var weight_sum = 0.0;
  for (var dy = -6; dy <= 6; dy += 1) {
    for (var dx = -6; dx <= 6; dx += 1) {
      let position = base + vec2<i32>(dx, dy);
      let weight_x = cubic((source.x - f32(position.x)) * scale.x) * scale.x;
      let weight_y = cubic((source.y - f32(position.y)) * scale.y) * scale.y;
      let weight = weight_x * weight_y;
      let mirrored = vec2<i32>(mirror(position.x, i32(input_size.x)), mirror(position.y, i32(input_size.y)));
      sum += textureLoad(texture, mirrored, 0).x * weight;
      weight_sum += weight;
    }
  }
  return sum / max(weight_sum, 1e-12);
}

@compute @workgroup_size(8, 8)
fn drc_prefilter_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (params.level_count == 0u || id.x >= size.x || id.y >= size.y) { return; }
  let center = vec2<i32>(id.xy);
  let weights = array<f32, 3>(1.0, 2.0, 1.0);
  var sum = 0.0;
  for (var dy = -1; dy <= 1; dy += 1) {
    for (var dx = -1; dx <= 1; dx += 1) {
      let position = center + vec2<i32>(dx, dy);
      sum += load_zero(input_a, position).x * weights[u32(dx + 1)] * weights[u32(dy + 1)];
    }
  }
  textureStore(output_r32, center, vec4<f32>(sum / 16.0, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn pyramid_downsample_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (params.level_count == 0u || id.x >= size.x || id.y >= size.y) { return; }
  textureStore(output_r32, vec2<i32>(id.xy), vec4<f32>(resize_sample(input_a, id.xy, size), 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn pyramid_reconstruct_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (params.level_count == 0u || id.x >= size.x || id.y >= size.y) { return; }
  let fine = textureLoad(input_a, vec2<i32>(id.xy), 0).x;
  let coarse = resize_sample(input_b, id.xy, size);
  let base = resize_sample(input_c, id.xy, size);
  textureStore(output_r32, vec2<i32>(id.xy), vec4<f32>(base + fine - coarse, 0.0, 0.0, 0.0));
}


fn gradient_chi(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  return gf_gradient_chi(texture, center);
}

fn gradient_weight(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  return gf_gradient_weight(texture, center);
}

fn gradient_guided_gamma(texture: texture_2d<f32>, center: vec2<i32>) -> f32 {
  return gf_gradient_gamma(texture, center);
}

@compute @workgroup_size(8, 8)
fn guided_coefficients_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_rgba);
  if (params.level_count == 0u || id.x >= size.x || id.y >= size.y) { return; }
  let position = vec2<i32>(id.xy);
  let moments = gf_box_moments(input_a, position, vec2<i32>(size));
  let variance = max(moments.y - moments.x * moments.x, 0.0);
  let weight = max(gf_gradient_weight(input_a, position), 1e-6);
  let regularization = 1.0 / weight;
  let gamma = gf_gradient_gamma(input_a, position);
  let a = (variance + regularization * gamma) / (variance + regularization);
  let b = moments.x - a * moments.x;
  textureStore(output_rgba, position, vec4<f32>(a, b, 0.0, 0.0));
}

fn load_clamped_r32(texture: texture_2d<f32>, position: vec2<i32>) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  return textureLoad(texture, clamp(position, vec2<i32>(0), size - vec2<i32>(1)), 0).x;
}

fn lookup_modulation(base: u32, count: u32, value: f32) -> f32 {
  let position = clamp(value, 0.0, 1.0) * f32(count - 1u);
  let lower = min(u32(floor(position)), count - 1u);
  let upper = min(lower + 1u, count - 1u);
  return mix(modulation_luts.values[base + lower], modulation_luts.values[base + upper], position - f32(lower));
}

fn edge_curve(value: f32) -> f32 {
  return lookup_modulation(0u, 64u, value);
}

fn sobel_magnitude(texture: texture_2d<f32>, position: vec2<i32>) -> f32 {
  let gx = load_clamped_r32(texture, position + vec2<i32>(1, -1))
    + 2.0 * load_clamped_r32(texture, position + vec2<i32>(1, 0))
    + load_clamped_r32(texture, position + vec2<i32>(1, 1))
    - load_clamped_r32(texture, position + vec2<i32>(-1, -1))
    - 2.0 * load_clamped_r32(texture, position + vec2<i32>(-1, 0))
    - load_clamped_r32(texture, position + vec2<i32>(-1, 1));
  let gy = load_clamped_r32(texture, position + vec2<i32>(-1, 1))
    + 2.0 * load_clamped_r32(texture, position + vec2<i32>(0, 1))
    + load_clamped_r32(texture, position + vec2<i32>(1, 1))
    - load_clamped_r32(texture, position + vec2<i32>(-1, -1))
    - 2.0 * load_clamped_r32(texture, position + vec2<i32>(0, -1))
    - load_clamped_r32(texture, position + vec2<i32>(1, -1));
  return sqrt(gx * gx + gy * gy) / 8.0;
}

fn edge_mask_smoothed(texture: texture_2d<f32>, position: vec2<i32>) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  let weights = array<f32, 3>(1.0, 2.0, 1.0);
  var sum = 0.0;
  for (var dy = -1; dy <= 1; dy += 1) {
    for (var dx = -1; dx <= 1; dx += 1) {
      let sample_position = position + vec2<i32>(dx, dy);
      if (sample_position.x >= 0 && sample_position.y >= 0 && sample_position.x < size.x && sample_position.y < size.y) {
        sum += edge_curve(sobel_magnitude(texture, sample_position)) * weights[u32(dx + 1)] * weights[u32(dy + 1)];
      }
    }
  }
  return sum / 16.0;
}

fn luma_curve(value: f32) -> f32 {
  return lookup_modulation(64u, 64u, value);
}

fn luma_mask_smoothed(texture: texture_2d<f32>, position: vec2<i32>) -> f32 {
  let size = vec2<i32>(textureDimensions(texture));
  let weights = array<f32, 3>(1.0, 2.0, 1.0);
  var sum = 0.0;
  for (var dy = -1; dy <= 1; dy += 1) {
    for (var dx = -1; dx <= 1; dx += 1) {
      let sample_position = position + vec2<i32>(dx, dy);
      if (sample_position.x >= 0 && sample_position.y >= 0 && sample_position.x < size.x && sample_position.y < size.y) {
        sum += luma_curve(textureLoad(texture, sample_position, 0).x) * weights[u32(dx + 1)] * weights[u32(dy + 1)];
      }
    }
  }
  return sum / 16.0;
}

@compute @workgroup_size(8, 8)
fn guided_apply_vertical_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (params.level_count == 0u || id.x >= size.x || id.y >= size.y) { return; }
  var sum = vec2<f32>(0.0);
  let x_lo = max(i32(id.x) - 3, 0);
  let x_hi = min(i32(id.x) + 3, i32(size.x) - 1);
  let y_lo = max(i32(id.y) - 3, 0);
  let y_hi = min(i32(id.y) + 3, i32(size.y) - 1);
  for (var y = y_lo; y <= y_hi; y += 1) {
    for (var x = x_lo; x <= x_hi; x += 1) {
      sum += textureLoad(input_a, vec2<i32>(x, y), 0).rg;
    }
  }
  let count = f32((x_hi - x_lo + 1) * (y_hi - y_lo + 1));
  let position = vec2<i32>(id.xy);
  let guide = textureLoad(input_b, position, 0).x;
  let guided_value = sum.x / count * guide + sum.y / count;
  let edge_mask = edge_mask_smoothed(input_b, position);
  let result = mix(guide, guided_value, edge_mask);
  textureStore(output_r32, position, vec4<f32>(result, 0.0, 0.0, 0.0));
}

fn lookup_global(value: f32) -> f32 {
  let count = arrayLength(&global_lut.values);
  let position = clamp(value, 0.0, 1.0) * f32(count - 1u);
  let lower = min(u32(floor(position)), count - 1u);
  let upper = min(lower + 1u, count - 1u);
  return mix(global_lut.values[lower], global_lut.values[upper], position - f32(lower));
}

fn sample_local_tile(tile_x: u32, tile_y: u32, tiles_x: u32, count: u32, position: f32) -> f32 {
  let lower = min(u32(floor(position)), count - 1u);
  let upper = min(lower + 1u, count - 1u);
  let base = (tile_y * tiles_x + tile_x) * count;
  return mix(local_lut.values[base + lower], local_lut.values[base + upper], position - f32(lower));
}

fn lookup_local(value: f32, pixel: vec2<u32>, size: vec2<u32>) -> f32 {
  let count = arrayLength(&global_lut.values);
  let tiles_x = max((params.feature_flags >> 8u) & 0xffu, 1u);
  let tiles_y = max((params.feature_flags >> 16u) & 0xffu, 1u);
  let local_grid = (vec2<f32>(pixel) + vec2<f32>(0.5)) * vec2<f32>(f32(tiles_x), f32(tiles_y)) / vec2<f32>(size) - vec2<f32>(0.5);
  let lower_grid = vec2<i32>(floor(local_grid));
  let fraction = fract(local_grid);
  let x0 = u32(clamp(lower_grid.x, 0, i32(tiles_x) - 1));
  let y0 = u32(clamp(lower_grid.y, 0, i32(tiles_y) - 1));
  let x1 = min(x0 + 1u, tiles_x - 1u);
  let y1 = min(y0 + 1u, tiles_y - 1u);
  let position = clamp(value, 0.0, 1.0) * f32(count - 1u);
  let top = mix(sample_local_tile(x0, y0, tiles_x, count, position), sample_local_tile(x1, y0, tiles_x, count, position), fraction.x);
  let bottom = mix(sample_local_tile(x0, y1, tiles_x, count, position), sample_local_tile(x1, y1, tiles_x, count, position), fraction.x);
  return mix(top, bottom, fraction.y);
}

fn combine(pixel: vec2<u32>, mapped: f32) -> f32 {
  let position = vec2<i32>(pixel);
  let raw = textureLoad(input_a, position, 0).x;
  let luma = textureLoad(input_b, position, 0).x;
  let base = textureLoad(input_c, position, 0).x;
  if (luma <= params.luma_guard) { return raw; }
  let detail = luma - base;
  let luma_mask = luma_mask_smoothed(input_b, position);
  let target_value = mapped + params.amplifier * detail * luma_mask;
  return clamp(raw * clamp(target_value / luma, params.min_ratio, params.max_ratio), 0.0, 1.0);
}

@compute @workgroup_size(8, 8)
fn drc_combine_global_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (id.x >= size.x || id.y >= size.y) { return; }
  let luma = textureLoad(input_b, vec2<i32>(id.xy), 0).x;
  textureStore(output_r32, vec2<i32>(id.xy), vec4<f32>(combine(id.xy, lookup_global(luma)), 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn drc_combine_local_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(output_r32);
  if (id.x >= size.x || id.y >= size.y) { return; }
  let position = vec2<i32>(id.xy);
  let base = textureLoad(input_c, position, 0).x;
  let luma = textureLoad(input_b, position, 0).x;
  let global = lookup_global(base);
  let local = lookup_local(base, id.xy, size);
  let highlight_protection = smoothstep(0.02, 0.15, base) * (1.0 - smoothstep(0.82, 0.98, base));
  let detail_protection = 1.0 - clamp(abs(luma - base) * 2.0, 0.0, 0.75);
  let mapped = mix(global, local, highlight_protection * detail_protection);
  textureStore(output_r32, position, vec4<f32>(combine(id.xy, mapped), 0.0, 0.0, 0.0));
}
