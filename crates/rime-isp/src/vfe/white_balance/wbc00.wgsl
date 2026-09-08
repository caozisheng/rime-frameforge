struct WhiteBalanceParams {
  gains: vec4<f32>,
  cfa_pattern: vec4<u32>,
  highlight_recovery: u32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: WhiteBalanceParams;

// Highlight recovery reconstructs a sample whose post-gain value reaches
// saturation from the unclipped same-CFA-phase neighbors (stride 2). Samples
// with no unclipped neighbor keep the plain gain result.
fn recovered_value(position: vec2<i32>, channel: u32, balanced: f32) -> f32 {
  if (params.highlight_recovery == 0u || balanced <= 1.0) { return balanced; }
  let extent = vec2<i32>(textureDimensions(input_tex));
  var sum = 0.0;
  var count = 0u;
  for (var dy = -2; dy <= 2; dy += 2) {
    for (var dx = -2; dx <= 2; dx += 2) {
      if (dx == 0 && dy == 0) { continue; }
      let neighbor = position + vec2<i32>(dx, dy);
      if (neighbor.x < 0 || neighbor.y < 0 || neighbor.x >= extent.x || neighbor.y >= extent.y) { continue; }
      let phase = u32(neighbor.y & 1) * 2u + u32(neighbor.x & 1);
      if (params.cfa_pattern[phase] != channel) { continue; }
      let value = textureLoad(input_tex, neighbor, 0).r * params.gains[params.cfa_pattern[phase]];
      if (value <= 1.0) {
        sum += value;
        count += 1u;
      }
    }
  }
  if (count == 0u) { return balanced; }
  return sum / f32(count);
}

@compute @workgroup_size(8, 8)
fn wbc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let position = vec2<i32>(gid.xy);
  let phase = vec2<u32>(gid.x & 1u, gid.y & 1u);
  let channel = params.cfa_pattern[phase.y * 2u + phase.x];
  let gain = params.gains[channel];
  let value = textureLoad(input_tex, position, 0).r * gain;
  textureStore(output_tex, position, vec4<f32>(recovered_value(position, channel, value), 0.0, 0.0, 1.0));
}
