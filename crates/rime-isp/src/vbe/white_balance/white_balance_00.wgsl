@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn wbc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let phase = vec2<u32>(gid.x & 1u, gid.y & 1u);
  var gain = 1.0;
  if (phase.x == 0u && phase.y == 0u) { gain = 2.0; }
  if (phase.x == 1u && phase.y == 1u) { gain = 1.5; }
  let value = textureLoad(input_tex, vec2<i32>(gid.xy), 0).r * gain;
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(value, 0.0, 0.0, 1.0));
}
