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
  // The declared white level is a calibration reference, not a hard sensor
  // limit: some DNGs (GH5S 14-bit, WhiteLevel 8000, samples reaching 16383)
  // carry samples beyond it. Without clamping, the saturated platform lands
  // far above 1.0 and every downstream contract (HR clip detection, DRC
  // [0,1] knee, quantizer range) breaks on that fixture.
  let normalized = clamp((code - params.black_level) / (params.white_level - params.black_level), 0.0, 1.0);
  textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(normalized, 0.0, 0.0, 1.0));
}
