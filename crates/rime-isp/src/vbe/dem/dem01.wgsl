struct DemosaicParams {
  cfa_pattern: vec4<u32>,
  thresholds: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: DemosaicParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba32float, write>;

fn sample(p: vec2<i32>, extent: vec2<u32>) -> f32 {
  let hi = vec2<i32>(extent) - vec2<i32>(1);
  return textureLoad(input_tex, clamp(p, vec2<i32>(0), hi), 0).r;
}
fn cfa(p: vec2<i32>, extent: vec2<u32>) -> u32 {
  let q = clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1);
  let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u);
  return params.cfa_pattern[phase.y * 2u + phase.x];
}
fn kernel(p: vec2<i32>, extent: vec2<u32>, k: u32) -> f32 {
  var sum = 0.0;
  for (var dy: i32 = -2; dy <= 2; dy++) {
    for (var dx: i32 = -2; dx <= 2; dx++) {
      var w = 0.0;
      if (k == 0u) {
        if (dx == 0 && dy == -2) { w = -1.0; }
        if (dx == 0 && dy == -1) { w = 2.0; }
        if (dx == -2 && dy == 0 || dx == 2 && dy == 0) { w = -1.0; }
        if (dx == -1 && dy == 0 || dx == 1 && dy == 0) { w = 2.0; }
        if (dx == 0 && dy == 0) { w = 4.0; }
        if (dx == 0 && dy == 1) { w = 2.0; }
        if (dx == 0 && dy == 2) { w = -1.0; }
      } else if (k == 1u) {
        if (dx == 0 && abs(dy) == 2 || abs(dx) == 2 && dy == 0) { w = -3.0; }
        if (abs(dx) == 1 && abs(dy) == 1) { w = 4.0; }
        if (dx == 0 && dy == 0) { w = 12.0; }
      } else if (k == 2u) {
        if (dx == 0 && abs(dy) == 2) { w = 1.0; }
        if (abs(dx) == 1 && abs(dy) == 1) { w = -2.0; }
        if (abs(dx) == 2 && dy == 0) { w = -2.0; }
        if (abs(dx) == 1 && dy == 0) { w = 8.0; }
        if (dx == 0 && dy == 0) { w = 10.0; }
      } else {
        if (dx == 0 && abs(dy) == 2) { w = -2.0; }
        if (abs(dx) == 1 && abs(dy) == 1) { w = -2.0; }
        if (dx == 0 && abs(dy) == 1) { w = 8.0; }
        if (abs(dx) == 2 && dy == 0) { w = 1.0; }
        if (dx == 0 && dy == 0) { w = 10.0; }
      }
      sum += w * sample(p + vec2<i32>(dx, dy), extent);
    }
  }
  return sum / select(8.0, 16.0, k != 0u);
}

@compute @workgroup_size(8, 8)
fn demosaic_mhc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  let ch = cfa(p, extent);
  let center = sample(p, extent);
  var rgb = vec3<f32>(center);
  if (ch == 0u) {
    rgb.g = kernel(p, extent, 0u);
    rgb.b = kernel(p, extent, 1u);
  } else if (ch == 2u) {
    rgb.g = kernel(p, extent, 0u);
    rgb.r = kernel(p, extent, 1u);
  } else {
    let row_color = cfa(vec2<i32>(p.x ^ 1, p.y), extent);
    if (row_color == 0u) {
      rgb.r = kernel(p, extent, 2u);
      rgb.b = kernel(p, extent, 3u);
    } else {
      rgb.b = kernel(p, extent, 2u);
      rgb.r = kernel(p, extent, 3u);
    }
  }
  textureStore(output_tex, p, vec4<f32>(rgb, 1.0));
}
