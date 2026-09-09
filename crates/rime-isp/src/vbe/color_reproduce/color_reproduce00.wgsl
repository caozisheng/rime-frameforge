struct CrParams {
  dims_and_enable: vec4<u32>,
}

struct FloatBuffer { values: array<f32> }

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> params: CrParams;
@group(0) @binding(3) var<storage, read> cr_matrices: FloatBuffer;
@group(0) @binding(4) var<storage, read> cr_hsv_lut: FloatBuffer;

fn mat_entry(matrix: u32, row: u32, col: u32) -> f32 {
  return cr_matrices.values[matrix * 9u + row * 3u + col];
}

fn apply_matrix(matrix: u32, rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    mat_entry(matrix, 0u, 0u) * rgb.r + mat_entry(matrix, 0u, 1u) * rgb.g + mat_entry(matrix, 0u, 2u) * rgb.b,
    mat_entry(matrix, 1u, 0u) * rgb.r + mat_entry(matrix, 1u, 1u) * rgb.g + mat_entry(matrix, 1u, 2u) * rgb.b,
    mat_entry(matrix, 2u, 0u) * rgb.r + mat_entry(matrix, 2u, 1u) * rgb.g + mat_entry(matrix, 2u, 2u) * rgb.b,
  );
}

// MatRgb2Hsv: input clipped to [0,1]; R-priority on ties (MATLAB convention).
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
  let v = max(max(rgb.r, rgb.g), rgb.b);
  let c = v - min(min(rgb.r, rgb.g), rgb.b);
  var s = 0.0;
  if (v != 0.0) { s = c / v; }
  var h = 0.0;
  if (c != 0.0) {
    if (v == rgb.r) {
      h = 60.0 * (((rgb.g - rgb.b) / c) % 6.0);
    } else if (v == rgb.g) {
      h = 60.0 * ((rgb.b - rgb.r) / c + 2.0);
    } else {
      h = 60.0 * ((rgb.r - rgb.g) / c + 4.0);
    }
  }
  h = h % 360.0;
  return vec3<f32>(h, clamp(s, 0.0, 1.0), clamp(v, 0.0, 1.0));
}

// MatHsv2Rgb.
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
  let h = hsv.x % 360.0;
  let s = clamp(hsv.y, 0.0, 1.0);
  let v = clamp(hsv.z, 0.0, 1.0);
  let c = v * s;
  let x = c * (1.0 - abs((h / 60.0) % 2.0 - 1.0));
  var sector = vec3<f32>(0.0);
  let t = h / 60.0;
  if (t < 1.0) { sector = vec3<f32>(c, x, 0.0); }
  else if (t < 2.0) { sector = vec3<f32>(x, c, 0.0); }
  else if (t < 3.0) { sector = vec3<f32>(0.0, c, x); }
  else if (t < 4.0) { sector = vec3<f32>(0.0, x, c); }
  else if (t < 5.0) { sector = vec3<f32>(x, 0.0, c); }
  else { sector = vec3<f32>(c, 0.0, x); }
  return clamp(sector + vec3<f32>(v - c), vec3<f32>(0.0), vec3<f32>(1.0));
}

// MatHsvLookup: nearest-neighbour lookup in DNG grid order
// (v outer, hue middle, saturation inner; per-entry three floats).
fn hsv_lut_apply(hsv: vec3<f32>) -> vec3<f32> {
  if (params.dims_and_enable.w == 0u) { return hsv; }
  let hue_divs = params.dims_and_enable.x;
  let sat_divs = params.dims_and_enable.y;
  let val_divs = params.dims_and_enable.z;
  let h = min(u32(floor((hsv.x % 360.0) / 360.0 * f32(hue_divs))), hue_divs - 1u);
  let s = min(u32(floor(clamp(hsv.y, 0.0, 1.0) * f32(sat_divs))), sat_divs - 1u);
  let v = min(u32(floor(clamp(hsv.z, 0.0, 1.0) * f32(val_divs))), val_divs - 1u);
  let entry = (v * hue_divs + h) * sat_divs + s;
  let hue_shift = cr_hsv_lut.values[3u * entry + 0u];
  let sat_scale = cr_hsv_lut.values[3u * entry + 1u];
  let val_scale = cr_hsv_lut.values[3u * entry + 2u];
  return vec3<f32>(
    (hsv.x + hue_shift) % 360.0,
    clamp(hsv.y * sat_scale, 0.0, 1.0),
    clamp(hsv.z * val_scale, 0.0, 1.0),
  );
}

@compute @workgroup_size(8, 8, 1)
fn color_reproduce_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = textureDimensions(input_tex);
  if (gid.x >= extent.x || gid.y >= extent.y) { return; }
  let p = vec2<i32>(gid.xy);
  let sensor_rgb = textureLoad(input_tex, p, 0).rgb;

  // Step 1-2: sensor -> ProPhoto, per-channel clip (MATLAB step 7).
  var rgb = apply_matrix(0u, sensor_rgb);
  rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

  // Steps 3-6: HSV calibration round trip (MATLAB steps 8-10).
  var hsv = rgb_to_hsv(rgb);
  hsv = hsv_lut_apply(hsv);
  rgb = hsv_to_rgb(hsv);
  rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

  // Steps 7-8: ProPhoto -> sRGB, final per-channel clip (MATLAB steps 11-12).
  var srgb = apply_matrix(1u, rgb);
  srgb = clamp(srgb, vec3<f32>(0.0), vec3<f32>(1.0));

  textureStore(output_tex, p, vec4<f32>(srgb, 1.0));
}
