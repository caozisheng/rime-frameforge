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

@group(0) @binding(0) var yuv_tex: texture_2d<f32>;

@fragment
fn present_fragment(in: VertexOut) -> @location(0) vec4<f32> {
  let extent = textureDimensions(yuv_tex);
  let coordinate = vec2<i32>(clamp(in.uv * vec2<f32>(extent), vec2<f32>(0.0), vec2<f32>(extent - vec2<u32>(1u))));
  let yuv = textureLoad(yuv_tex, coordinate, 0).rgb;
  let y = yuv.x;
  let u = yuv.y - 0.5;
  let v = yuv.z - 0.5;
  let rgb = vec3<f32>(y + 1.5748 * v, y - 0.187324 * u - 0.468124 * v, y + 1.8556 * u);
  return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
