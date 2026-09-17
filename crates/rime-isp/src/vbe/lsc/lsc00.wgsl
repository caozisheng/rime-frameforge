struct LscParams {
  opcode_count: u32,
  width: u32,
  height: u32,
  _padding: u32,
};

struct VignetteRadialRecord {
  k0: f32,
  k1: f32,
  k2: f32,
  k3: f32,
  k4: f32,
  center_x: f32,
  center_y: f32,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: LscParams;
@group(0) @binding(3) var<storage, read> vignette_radial: array<VignetteRadialRecord>;

fn vignette_gain(record: VignetteRadialRecord, pixel: vec2<f32>, extent: vec2<f32>) -> f32 {
  let center = vec2<f32>(record.center_x, record.center_y) * extent;
  let delta = pixel - center;
  let farthest = max(center, extent - center);
  let maximum_radius_squared = max(dot(farthest, farthest), 1.0e-12);
  let radius_squared = dot(delta, delta) / maximum_radius_squared;
  return 1.0 + radius_squared * (record.k0 + radius_squared * (record.k1 + radius_squared * (record.k2 + radius_squared * (record.k3 + radius_squared * record.k4))));
}

@compute @workgroup_size(8, 8)
fn lsc_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let dimensions = textureDimensions(output_texture);
  if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
  let extent = vec2<f32>(f32(params.width), f32(params.height));
  let pixel = vec2<f32>(id.xy) + vec2<f32>(0.5);
  var corrected = textureLoad(input_texture, vec2<i32>(id.xy), 0).x;
  let output_max = 1.0 - 1.0 / 16384.0;
  for (var index = 0u; index < params.opcode_count; index += 1u) {
    corrected = clamp(corrected * vignette_gain(vignette_radial[index], pixel, extent), 0.0, output_max);
  }
  textureStore(output_texture, vec2<i32>(id.xy), vec4<f32>(corrected, 0.0, 0.0, 1.0));
}
