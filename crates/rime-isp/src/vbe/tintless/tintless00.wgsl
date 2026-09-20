struct TintlessParameters {
  source_extent: vec2<u32>,
  mesh_extent: vec2<u32>,
  cfa_pattern: vec4<u32>,
  gain_clamp: vec2<f32>,
  cold_start: u32,
  reserved: u32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: TintlessParameters;
@group(0) @binding(3) var<storage, read> gain_mesh: array<vec2<f32>, 3185>;

fn mesh_entry(x: u32, y: u32) -> vec2<f32> {
  return gain_mesh[y * params.mesh_extent.x + x];
}

fn interpolated_gain(pixel: vec2<u32>) -> vec2<f32> {
  let source_max = vec2<f32>(max(params.source_extent - vec2<u32>(1u), vec2<u32>(1u)));
  let mesh_max_u = params.mesh_extent - vec2<u32>(1u);
  let mesh_max = vec2<f32>(mesh_max_u);
  let position = vec2<f32>(pixel) * mesh_max / source_max;
  let lower = min(vec2<u32>(floor(position)), mesh_max_u);
  let upper = min(lower + vec2<u32>(1u), mesh_max_u);
  let fraction = position - vec2<f32>(lower);
  let top = mix(mesh_entry(lower.x, lower.y), mesh_entry(upper.x, lower.y), fraction.x);
  let bottom = mix(mesh_entry(lower.x, upper.y), mesh_entry(upper.x, upper.y), fraction.x);
  return clamp(mix(top, bottom, fraction.y), params.gain_clamp.xx, params.gain_clamp.yy);
}

fn cfa_channel(pixel: vec2<u32>) -> u32 {
  return params.cfa_pattern[(pixel.y & 1u) * 2u + (pixel.x & 1u)];
}

@compute @workgroup_size(8, 8)
fn tintless_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.source_extent.x || id.y >= params.source_extent.y) {
    return;
  }
  let pixel = id.xy;
  let gains = interpolated_gain(pixel);
  let channel = cfa_channel(pixel);
  var gain = 1.0;
  if (channel == 0u) {
    gain = gains.x;
  } else if (channel == 2u) {
    gain = gains.y;
  }
  let corrected = textureLoad(input_texture, vec2<i32>(pixel), 0).x * gain;
  textureStore(output_texture, vec2<i32>(pixel), vec4<f32>(corrected, 0.0, 0.0, 1.0));
}
