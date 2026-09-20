struct LcstParameters {
  source_extent: vec2<u32>,
  cfa_pattern: vec4<u32>,
  d50_gains: vec4<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: LcstParameters;
@group(0) @binding(2) var<storage, read_write> average_rggb: array<vec4<f32>, 3072>;
@group(0) @binding(3) var<storage, read_write> luma_histogram: array<u32, 4096>;

fn partition_bounds(index: u32, parts: u32, size: u32) -> vec2<u32> {
  return vec2<u32>(index * size / parts, (index + 1u) * size / parts);
}

fn cfa_channel(position: vec2<u32>) -> u32 {
  let site = (position.y & 1u) * 2u + (position.x & 1u);
  let color = params.cfa_pattern[site];
  if (color == 0u) { return 0u; }
  if (color == 2u) { return 3u; }
  var red_site = 0u;
  for (var index = 0u; index < 4u; index += 1u) {
    if (params.cfa_pattern[index] == 0u) { red_site = index; }
  }
  return select(2u, 1u, site / 2u == red_site / 2u);
}

fn clamped_coordinate(value: i32, extent: u32) -> u32 {
  return u32(clamp(value, 0, i32(extent) - 1));
}

fn filtered_luma(position: vec2<u32>) -> f32 {
  let weights = array<f32, 3>(1.0, 2.0, 1.0);
  var sum = 0.0;
  for (var ky = 0u; ky < 3u; ky += 1u) {
    let y = clamped_coordinate(i32(position.y) + i32(ky) - 1, params.source_extent.y);
    for (var kx = 0u; kx < 3u; kx += 1u) {
      let x = clamped_coordinate(i32(position.x) + i32(kx) - 1, params.source_extent.x);
      let sample_position = vec2<u32>(x, y);
      let sample = textureLoad(input_texture, vec2<i32>(sample_position), 0).x;
      sum += sample * params.d50_gains[cfa_channel(sample_position)] * weights[kx] * weights[ky];
    }
  }
  return sum / 16.0;
}

@compute @workgroup_size(1, 1, 1)
fn lcst_average_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= 64u || id.y >= 48u) { return; }
  let xb = partition_bounds(id.x, 64u, params.source_extent.x);
  let yb = partition_bounds(id.y, 48u, params.source_extent.y);
  var sums = vec4<f32>(0.0);
  var counts = vec4<u32>(0u);
  for (var y = yb.x; y < yb.y; y += 1u) {
    for (var x = xb.x; x < xb.y; x += 1u) {
      let channel = cfa_channel(vec2<u32>(x, y));
      sums[channel] += textureLoad(input_texture, vec2<i32>(i32(x), i32(y)), 0).x;
      counts[channel] += 1u;
    }
  }
  average_rggb[id.y * 64u + id.x] = sums / vec4<f32>(counts);
}

@compute @workgroup_size(1, 1, 1)
fn lcst_histogram_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= 16u || id.y >= 16u) { return; }
  let xb = partition_bounds(id.x, 16u, params.source_extent.x);
  let yb = partition_bounds(id.y, 16u, params.source_extent.y);
  var bins = array<u32, 16>();
  for (var y = yb.x; y < yb.y; y += 1u) {
    for (var x = xb.x; x < xb.y; x += 1u) {
      let luma = clamp(filtered_luma(vec2<u32>(x, y)), 0.0, 1.0);
      let bin = min(u32(luma * 16.0), 15u);
      bins[bin] += 1u;
    }
  }
  let tile = id.y * 16u + id.x;
  for (var bin = 0u; bin < 16u; bin += 1u) {
    luma_histogram[tile * 16u + bin] = bins[bin];
  }
}
