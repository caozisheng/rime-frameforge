// DEM method 00 — bilinear demosaic.
//
// Composable operator convention (see AGENTS.md): bindings use the
// standard names (`input_tex`, `params`, `output_tex`) so executors can
// splice this file's functions into a fused shell with a single
// controlled alias. The `dem00_sample` function is the composable
// sampling entry (no bindings of its own); the compute entry below is
// the native standalone form.

struct DemosaicParams {
  cfa_pattern: vec4<u32>,
  thresholds: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: DemosaicParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba32float, write>;

fn dem00_fetch(p: vec2<i32>, extent: vec2<u32>) -> f32 {
  let hi = vec2<i32>(extent) - vec2<i32>(1);
  return textureLoad(input_tex, clamp(p, vec2<i32>(0), hi), 0).r;
}

fn dem00_cfa(p: vec2<i32>, extent: vec2<u32>) -> u32 {
  let q = clamp(p, vec2<i32>(0), vec2<i32>(extent) - 1);
  let phase = vec2<u32>(u32(q.x) & 1u, u32(q.y) & 1u);
  return params.cfa_pattern[phase.y * 2u + phase.x];
}

// Peak normalization: when the demosaic average peaks above 1.0, scale
// all channels by 1/peak — an equal-ratio rescale that PRESERVES hue,
// is continuous at peak -> 1, and folds the white point to 1.0. (A
// reciprocal knee like rgb/(1+excess) is discontinuous at the boundary
// and darkens ordinary highlights whose R sites legitimately exceed 1.0
// in the divided domain — do not reintroduce it.)
fn shared_saturation_clip(rgb: vec3<f32>) -> vec3<f32> {
  let peak = max(max(rgb.r, rgb.g), rgb.b);
  let scaled = rgb / peak;
  let clipped = select(scaled, vec3<f32>(1.0), min(min(scaled.r, scaled.g), scaled.b) >= 0.5);
  return select(rgb, clipped, peak > 1.0);
}

/// Per-pixel bilinear RGB estimate (composable sampling entry).
fn dem00_sample(p: vec2<i32>, extent: vec2<u32>) -> vec4<f32> {
  var sums = vec3<f32>(0.0);
  var counts = vec3<f32>(0.0);
  let low = max(p - vec2<i32>(1), vec2<i32>(0));
  let high = min(p + vec2<i32>(1), vec2<i32>(extent) - 1);
  for (var y = low.y; y <= high.y; y++) {
    for (var x = low.x; x <= high.x; x++) {
      let q = vec2<i32>(x, y);
      let channel = dem00_cfa(q, extent);
      sums[channel] += dem00_fetch(q, extent);
      counts[channel] += 1.0;
    }
  }
  let rgb = sums / max(counts, vec3<f32>(1.0));
  return vec4<f32>(shared_saturation_clip(rgb), 1.0);
}

@compute @workgroup_size(8, 8)
fn demosaic_bilinear_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  textureStore(output_tex, p, dem00_sample(p, extent));
}
