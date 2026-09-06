struct GuidedParams {
  radius: u32,
  width: u32,
  height: u32,
  epsilon: f32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var guide_texture: texture_2d<f32>;
@group(0) @binding(2) var auxiliary_texture: texture_2d<f32>;
@group(0) @binding(3) var output_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var<uniform> params: GuidedParams;

fn inside(x: i32, y: i32) -> bool {
  return x >= 0 && y >= 0 && x < i32(params.width) && y < i32(params.height);
}

@compute @workgroup_size(8, 8)
fn guided_box_horizontal_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.width || id.y >= params.height) { return; }
  var sum = vec4<f32>(0.0);
  var count = 0.0;
  for (var dx = -i32(params.radius); dx <= i32(params.radius); dx += 1) {
    let x = i32(id.x) + dx;
    if (inside(x, i32(id.y))) {
      sum += textureLoad(input_texture, vec2<i32>(x, i32(id.y)), 0);
      count += 1.0;
    }
  }
  textureStore(output_texture, vec2<i32>(id.xy), sum / count);
}

@compute @workgroup_size(8, 8)
fn guided_box_vertical_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.width || id.y >= params.height) { return; }
  var sum = vec4<f32>(0.0);
  var count = 0.0;
  for (var dy = -i32(params.radius); dy <= i32(params.radius); dy += 1) {
    let y = i32(id.y) + dy;
    if (inside(i32(id.x), y)) {
      sum += textureLoad(input_texture, vec2<i32>(i32(id.x), y), 0);
      count += 1.0;
    }
  }
  textureStore(output_texture, vec2<i32>(id.xy), sum / count);
}

@compute @workgroup_size(8, 8)
fn guided_coefficients_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.width || id.y >= params.height) { return; }
  let xy = vec2<i32>(id.xy);
  let moments = textureLoad(input_texture, xy, 0);
  let mean_values = textureLoad(auxiliary_texture, xy, 0);
  let variance = max(moments.z - mean_values.x * mean_values.x, 0.0);
  let covariance = moments.w - mean_values.x * mean_values.y;
  let a = covariance / max(variance + params.epsilon, 1e-12);
  let b = mean_values.y - a * mean_values.x;
  textureStore(output_texture, xy, vec4<f32>(a, b, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn guided_apply_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.width || id.y >= params.height) { return; }
  let xy = vec2<i32>(id.xy);
  let coefficients = textureLoad(auxiliary_texture, xy, 0);
  let guide = textureLoad(guide_texture, xy, 0).x;
  let value = coefficients.x * guide + coefficients.y;
  textureStore(output_texture, xy, vec4<f32>(value, 0.0, 0.0, 0.0));
}
