@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8)
fn rgb2yuv_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let rgb = textureLoad(input_tex, vec2<i32>(gid.xy), 0).rgb;
  let y = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
  let u = dot(rgb, vec3<f32>(-0.114572, -0.385428, 0.5)) + 0.5;
  let v = dot(rgb, vec3<f32>(0.5, -0.454153, -0.045847)) + 0.5;
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(y, u, v, 1.0));
}
