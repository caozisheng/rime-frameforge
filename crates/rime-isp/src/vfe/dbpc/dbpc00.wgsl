@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn identity_r32_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let dimensions = textureDimensions(output_texture);
  if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
  textureStore(output_texture, vec2<i32>(id.xy), textureLoad(input_texture, vec2<i32>(id.xy), 0));
}
