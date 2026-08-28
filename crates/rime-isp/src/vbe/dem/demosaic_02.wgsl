struct DemosaicParams {
  cfa_pattern: vec4<u32>,
  thresholds: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: DemosaicParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba32float, write>;

fn sample(p: vec2<i32>, extent: vec2<u32>) -> f32 {
  return textureLoad(input_tex, clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1), 0).r;
}
fn cfa(p: vec2<i32>, extent: vec2<u32>) -> u32 {
  let q = clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1);
  let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u);
  return params.cfa_pattern[phase.y * 2u + phase.x];
}
fn green_at(p: vec2<i32>, extent: vec2<u32>) -> f32 {
  let position = clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1);
  let ch = cfa(position, extent);
  if (ch == 1u) { return sample(position, extent); }
  let center = sample(position, extent);
  let left = sample(position + vec2<i32>(-1, 0), extent);
  let right = sample(position + vec2<i32>(1, 0), extent);
  let top = sample(position + vec2<i32>(0, -1), extent);
  let bottom = sample(position + vec2<i32>(0, 1), extent);
  let left2 = sample(position + vec2<i32>(-2, 0), extent);
  let right2 = sample(position + vec2<i32>(2, 0), extent);
  let top2 = sample(position + vec2<i32>(0, -2), extent);
  let bottom2 = sample(position + vec2<i32>(0, 2), extent);
  let horizontal_gradient = abs(left - right) + abs(2.0 * center - left2 - right2);
  let vertical_gradient = abs(top - bottom) + abs(2.0 * center - top2 - bottom2);
  let horizontal = 0.5 * (left + right) + 0.25 * (2.0 * center - left2 - right2);
  let vertical = 0.5 * (top + bottom) + 0.25 * (2.0 * center - top2 - bottom2);
  if (horizontal_gradient < vertical_gradient) { return horizontal; }
  if (vertical_gradient < horizontal_gradient) { return vertical; }
  return 0.5 * (horizontal + vertical);
}
fn cardinal_difference(p: vec2<i32>, extent: vec2<u32>, horizontal: bool) -> f32 {
  let position = clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1);
  let first = select(position + vec2<i32>(0, -1), position + vec2<i32>(-1, 0), horizontal);
  let second = select(position + vec2<i32>(0, 1), position + vec2<i32>(1, 0), horizontal);
  return 0.5 * (sample(first, extent) - green_at(first, extent) + sample(second, extent) - green_at(second, extent));
}
fn diagonal_difference(p: vec2<i32>, extent: vec2<u32>, target_channel: u32) -> f32 {
  var sum = 0.0;
  var count = 0.0;
  for (var dy: i32 = -1; dy <= 1; dy += 2) {
    for (var dx: i32 = -1; dx <= 1; dx += 2) {
      let q = p + vec2<i32>(dx, dy);
      if (cfa(q, extent) == target_channel) {
        sum += sample(q, extent) - green_at(q, extent);
        count += 1.0;
      }
    }
  }
  return sum / max(count, 1.0);
}

@compute @workgroup_size(8, 8)
fn demosaic_ppg_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  let channel = cfa(p, extent);
  let green = green_at(p, extent);
  var rgb = vec3<f32>(green);
  if (channel == 0u) {
    rgb.r = sample(p, extent);
    rgb.b = green + diagonal_difference(p, extent, 2u);
  } else if (channel == 2u) {
    rgb.r = green + diagonal_difference(p, extent, 0u);
    rgb.b = sample(p, extent);
  } else {
    let row_color = cfa(vec2<i32>(p.x ^ 1, p.y), extent);
    if (row_color == 0u) {
      rgb.r = green + cardinal_difference(p, extent, true);
      rgb.b = green + cardinal_difference(p, extent, false);
    } else {
      rgb.b = green + cardinal_difference(p, extent, true);
      rgb.r = green + cardinal_difference(p, extent, false);
    }
  }
  textureStore(output_tex, p, vec4<f32>(rgb, 1.0));
}
