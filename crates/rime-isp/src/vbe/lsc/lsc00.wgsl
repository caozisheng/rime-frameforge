// LSC method 00 — unified gain-mesh shading correction.
//
// Consumes the frozen LSC packet from preprocess:
//   binding 2 (uniform): mesh_count + cfa phase (4x u32).
//   binding 3 (storage, read): packed mesh headers, 56 bytes each —
//     points_v, points_h, planes, entries_offset (f32 units),
//     spacing_v, spacing_h, origin_v, origin_h (f32),
//     area_t, area_l, area_b, area_r (i32, exclusive bottom/right),
//     row_pitch, col_pitch (u32, at least 1).
//   binding 4 (storage, read): concatenated f32 mesh entries,
//     row-major [row][col][plane].
//
// Interpolation contract (DNG SDK semantics, shared with `lsc_common`):
// pixel-center coordinate u = (id + 0.5) / extent; mesh position
// pos = (u - origin) / spacing clamped to the node grid; bilinear
// interpolation with plane = min(channel, planes - 1) where channel is
// the CFA phase channel — a 1-plane mesh is camera-agnostic, a 3-plane
// mesh carries per-color shading. Pixels outside the header's area
// bounds — or off the pitch lattice anchored at the area's top/left
// corner — receive identity — DNG `dng_area_spec` semantics.

struct LscParams {
  mesh_count: u32,
  cfa_r: u32,
  cfa_g: u32,
  cfa_b: u32,
};

struct MeshHeader {
  points_v: u32,
  points_h: u32,
  planes: u32,
  entries_offset: u32,
  spacing_v: f32,
  spacing_h: f32,
  origin_v: f32,
  origin_h: f32,
  area_t: i32,
  area_l: i32,
  area_b: i32,
  area_r: i32,
  row_pitch: u32,
  col_pitch: u32,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: LscParams;
@group(0) @binding(3) var<storage, read> mesh_headers: array<MeshHeader>;
@group(0) @binding(4) var<storage, read> mesh_entries: array<f32>;

fn cfa_channel(phase: vec2<u32>) -> u32 {
  let linear = phase.y * 2u + phase.x;
  let code = select(select(params.cfa_r, params.cfa_g, linear == 1u),
                    select(params.cfa_g, params.cfa_b, linear == 3u),
                    linear >= 2u);
  return code;
}

fn mesh_entry(header: MeshHeader, row: u32, col: u32, plane: u32) -> f32 {
  let index = header.entries_offset + row * header.points_h * header.planes
    + col * header.planes + plane;
  return mesh_entries[index];
}

fn mesh_gain(header: MeshHeader, channel: u32, id: vec2<u32>, pixel: vec2<f32>) -> f32 {
  let inside = i32(id.y) >= header.area_t && i32(id.y) < header.area_b
    && i32(id.x) >= header.area_l && i32(id.x) < header.area_r
    && (id.y - u32(header.area_t)) % header.row_pitch == 0u
    && (id.x - u32(header.area_l)) % header.col_pitch == 0u;
  if (!inside) { return 1.0; }
  let extent = vec2<f32>(textureDimensions(output_texture));
  let normalized = pixel / extent;
  let column_position = (normalized.x - header.origin_h) / header.spacing_h;
  let row_position = (normalized.y - header.origin_v) / header.spacing_v;

  let plane = min(channel, header.planes - 1u);
  let column = clamp(column_position, 0.0, f32(header.points_h - 1u));
  let row = clamp(row_position, 0.0, f32(header.points_v - 1u));
  let col0 = u32(column);
  let row0 = u32(row);
  let col1 = min(col0 + 1u, header.points_h - 1u);
  let row1 = min(row0 + 1u, header.points_v - 1u);
  let fx = column - f32(col0);
  let fy = row - f32(row0);

  let top = mesh_entry(header, row0, col0, plane)
    + (mesh_entry(header, row0, col1, plane) - mesh_entry(header, row0, col0, plane)) * fx;
  let bottom = mesh_entry(header, row1, col0, plane)
    + (mesh_entry(header, row1, col1, plane) - mesh_entry(header, row1, col0, plane)) * fx;
  return top + (bottom - top) * fy;
}

@compute @workgroup_size(8, 8)
fn lsc_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let dimensions = textureDimensions(output_texture);
  if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
  if (params.mesh_count == 0u) {
    // No correction sources: identity copy (executor normally bypasses).
    let raw = textureLoad(input_texture, vec2<i32>(id.xy), 0).x;
    textureStore(output_texture, vec2<i32>(id.xy), vec4<f32>(raw, 0.0, 0.0, 1.0));
    return;
  }

  let pixel = vec2<f32>(id.xy) + vec2<f32>(0.5);
  let channel = cfa_channel(id.xy % 2u);
  var corrected = textureLoad(input_texture, vec2<i32>(id.xy), 0).x;
  let output_max = 1.0 - 1.0 / 16384.0;
  for (var index = 0u; index < params.mesh_count; index += 1u) {
    corrected = clamp(corrected * mesh_gain(mesh_headers[index], channel, id.xy, pixel), 0.0, output_max);
  }
  textureStore(output_texture, vec2<i32>(id.xy), vec4<f32>(corrected, 0.0, 0.0, 1.0));
}
