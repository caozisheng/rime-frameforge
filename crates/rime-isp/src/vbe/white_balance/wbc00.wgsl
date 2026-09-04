struct WhiteBalanceParams {
  gains: vec4<f32>,
  cfa_pattern: vec4<u32>,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: WhiteBalanceParams;


@compute @workgroup_size(8, 8)
fn wbc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let phase = vec2<u32>(gid.x & 1u, gid.y & 1u);
  let channel = params.cfa_pattern[phase.y * 2u + phase.x];
  let gain = params.gains[channel];
  let value = textureLoad(input_tex, vec2<i32>(gid.xy), 0).r * gain;
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(value, 0.0, 0.0, 1.0));
}
