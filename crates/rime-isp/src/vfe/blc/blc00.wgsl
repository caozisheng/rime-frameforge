struct BlcParams {
  black_level: f32,
  white_level: f32,
  width: u32,
  height: u32,
}

@group(0) @binding(0) var<uniform> params: BlcParams;
@group(0) @binding(1) var input_tex: texture_2d<u32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn blc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let code = f32(textureLoad(input_tex, vec2<i32>(gid.xy), 0).r);
  let normalized = (code - params.black_level) / (params.white_level - params.black_level);
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(normalized, 0.0, 0.0, 1.0));
}
