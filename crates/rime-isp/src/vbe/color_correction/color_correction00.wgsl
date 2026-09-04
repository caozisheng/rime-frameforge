@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8)
fn color_correction_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let rgb = textureLoad(input_tex, vec2<i32>(gid.xy), 0).rgb;
  let corrected = vec3<f32>(
    1.08 * rgb.r - 0.04 * rgb.g - 0.04 * rgb.b,
    -0.03 * rgb.r + 1.06 * rgb.g - 0.03 * rgb.b,
    -0.02 * rgb.r - 0.06 * rgb.g + 1.08 * rgb.b,
  );
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(corrected, 1.0));
}
