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

@compute @workgroup_size(8, 8)
fn demosaic_bilinear_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  var sums = vec3<f32>(0.0);
  var counts = vec3<f32>(0.0);
  let low = max(p - vec2<i32>(1), vec2<i32>(0));
  let high = min(p + vec2<i32>(1), vec2<i32>(extent) - 1);
  for (var y = low.y; y <= high.y; y++) {
    for (var x = low.x; x <= high.x; x++) {
      let q = vec2<i32>(x, y);
      let channel = cfa(q, extent);
      sums[channel] += sample(q, extent);
      counts[channel] += 1.0;
    }
  }
  let rgb = sums / max(counts, vec3<f32>(1.0));
  textureStore(output_tex, p, vec4<f32>(rgb, 1.0));
}
