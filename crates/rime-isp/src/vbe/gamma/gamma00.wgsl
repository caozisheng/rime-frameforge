struct GammaParams {
  gamma_and_padding: vec4<f32>,
  lut: array<vec4<f32>, 3>,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> params: GammaParams;

fn lut_value(index: u32) -> f32 {
  return params.lut[index / 4u][index % 4u];
}

fn lut_secant(index: u32) -> f32 {
  return lut_value(index + 1u) - lut_value(index);
}

fn lut_tangent(index: u32) -> f32 {
  if (index == 0u) {
    return lut_secant(0u);
  }
  if (index >= 8u) {
    return lut_secant(7u);
  }
  let left = lut_secant(index - 1u);
  let right = lut_secant(index);
  if (left * right <= 0.0) {
    return 0.0;
  }
  return 2.0 * left * right / (left + right);
}

fn sample_luminance_lut(value: f32) -> f32 {
  if (value > 1.0) {
    return value;
  }
  let coordinate = clamp(value, 0.0, 1.0) * 8.0;
  let index = min(u32(floor(coordinate)), 7u);
  let t = coordinate - f32(index);
  let y0 = lut_value(index);
  let y1 = lut_value(index + 1u);
  let control1 = y0 + lut_tangent(index) / 3.0;
  let control2 = y1 - lut_tangent(index + 1u) / 3.0;
  let one_minus_t = 1.0 - t;
  return clamp(
    one_minus_t * one_minus_t * one_minus_t * y0
      + 3.0 * one_minus_t * one_minus_t * t * control1
      + 3.0 * one_minus_t * t * t * control2
      + t * t * t * y1,
    min(y0, y1),
    max(y0, y1),
  );
}

@compute @workgroup_size(8, 8, 1)
fn gamma_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  let linear_rgb = max(textureLoad(input_tex, p, 0).rgb, vec3<f32>(0.0));
  let luminance = dot(linear_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
  var mapped_rgb = vec3<f32>(0.0);
  if (luminance > 0.000001) {
    let mapped_luminance = sample_luminance_lut(luminance);
    mapped_rgb = linear_rgb * (mapped_luminance / luminance);
  }
  let encoded = pow(max(mapped_rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / max(params.gamma_and_padding.x, 0.000001)));
  textureStore(output_tex, p, vec4<f32>(encoded, 1.0));
}
