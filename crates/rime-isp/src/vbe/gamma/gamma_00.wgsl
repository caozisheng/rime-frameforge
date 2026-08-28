@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8)
fn gamma_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let rgb = max(textureLoad(input_tex, vec2<i32>(gid.xy), 0).rgb, vec3<f32>(0.0));
  let encoded = pow(rgb, vec3<f32>(1.0 / 2.2));
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(encoded, 1.0));
}
