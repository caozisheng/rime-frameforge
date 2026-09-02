struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
}

@vertex
fn present_vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
  let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var out: VertexOut;
  out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
  out.uv = 0.5 * (positions[vertex_index] + vec2<f32>(1.0));
  out.uv.y = 1.0 - out.uv.y;
  return out;
}

@group(0) @binding(0) var float_tex: texture_2d<f32>;
@group(1) @binding(0) var uint_tex: texture_2d<u32>;

fn coordinate(uv: vec2<f32>, extent: vec2<u32>) -> vec2<i32> {
  return vec2<i32>(clamp(uv * vec2<f32>(extent), vec2<f32>(0.0), vec2<f32>(extent - vec2<u32>(1u))));
}

fn display_code(rgb: vec3<f32>) -> vec4<f32> {
  return vec4<f32>(clamp(trunc(rgb * 256.0), vec3<f32>(0.0), vec3<f32>(255.0)) / 255.0, 1.0);
}

@fragment
fn present_raw(in: VertexOut) -> @location(0) vec4<f32> {
  let extent = textureDimensions(uint_tex);
  let value = f32(textureLoad(uint_tex, coordinate(in.uv, extent), 0).r) / 65535.0;
  return vec4<f32>(value, value, value, 1.0);
}

@fragment
fn present_gray(in: VertexOut) -> @location(0) vec4<f32> {
  let extent = textureDimensions(float_tex);
  let value = textureLoad(float_tex, coordinate(in.uv, extent), 0).r;
  return display_code(vec3<f32>(value));
}

@fragment
fn present_rgb(in: VertexOut) -> @location(0) vec4<f32> {
  let extent = textureDimensions(float_tex);
  return display_code(textureLoad(float_tex, coordinate(in.uv, extent), 0).rgb);
}

@fragment
fn present_yuv(in: VertexOut) -> @location(0) vec4<f32> {
  let extent = textureDimensions(float_tex);
  let yuv = textureLoad(float_tex, coordinate(in.uv, extent), 0).rgb;
  let y = yuv.x;
  let u = yuv.y - 0.5;
  let v = yuv.z - 0.5;
  return display_code(vec3<f32>(y + 1.5748 * v, y - 0.187324 * u - 0.468124 * v, y + 1.8556 * u));
}
