export const lcstPipeline = {
  "averageBytes": 49152,
  "averageDispatch": [
    64,
    48,
    1
  ],
  "histogramBytes": 16384,
  "histogramDispatch": [
    16,
    16,
    1
  ],
  "method": "00",
  "payloadBytes": 65536,
  "stages": [
    {
      "bindings": [
        {
          "access": "read",
          "binding": 0,
          "kind": "texture",
          "resource": "input"
        },
        {
          "access": "read",
          "binding": 1,
          "kind": "uniform_buffer",
          "resource": "parameters"
        },
        {
          "access": "write",
          "binding": 2,
          "kind": "storage_buffer",
          "resource": "average_rggb"
        }
      ],
      "entryPoint": "lcst_average_main"
    },
    {
      "bindings": [
        {
          "access": "read",
          "binding": 0,
          "kind": "texture",
          "resource": "input"
        },
        {
          "access": "read",
          "binding": 1,
          "kind": "uniform_buffer",
          "resource": "parameters"
        },
        {
          "access": "write",
          "binding": 3,
          "kind": "storage_buffer",
          "resource": "luma_histogram"
        }
      ],
      "entryPoint": "lcst_histogram_main"
    }
  ],
  "wgsl": "struct LcstParameters {\n  source_extent: vec2<u32>,\n  cfa_pattern: vec4<u32>,\n  d50_gains: vec4<f32>,\n}\n\n@group(0) @binding(0) var input_texture: texture_2d<f32>;\n@group(0) @binding(1) var<uniform> params: LcstParameters;\n@group(0) @binding(2) var<storage, read_write> average_rggb: array<vec4<f32>, 3072>;\n@group(0) @binding(3) var<storage, read_write> luma_histogram: array<u32, 4096>;\n\nfn partition_bounds(index: u32, parts: u32, size: u32) -> vec2<u32> {\n  return vec2<u32>(index * size / parts, (index + 1u) * size / parts);\n}\n\nfn cfa_channel(position: vec2<u32>) -> u32 {\n  let site = (position.y & 1u) * 2u + (position.x & 1u);\n  let color = params.cfa_pattern[site];\n  if (color == 0u) { return 0u; }\n  if (color == 2u) { return 3u; }\n  var red_site = 0u;\n  for (var index = 0u; index < 4u; index += 1u) {\n    if (params.cfa_pattern[index] == 0u) { red_site = index; }\n  }\n  return select(2u, 1u, site / 2u == red_site / 2u);\n}\n\nfn clamped_coordinate(value: i32, extent: u32) -> u32 {\n  return u32(clamp(value, 0, i32(extent) - 1));\n}\n\nfn filtered_luma(position: vec2<u32>) -> f32 {\n  let weights = array<f32, 3>(1.0, 2.0, 1.0);\n  var sum = 0.0;\n  for (var ky = 0u; ky < 3u; ky += 1u) {\n    let y = clamped_coordinate(i32(position.y) + i32(ky) - 1, params.source_extent.y);\n    for (var kx = 0u; kx < 3u; kx += 1u) {\n      let x = clamped_coordinate(i32(position.x) + i32(kx) - 1, params.source_extent.x);\n      let sample_position = vec2<u32>(x, y);\n      let sample = textureLoad(input_texture, vec2<i32>(sample_position), 0).x;\n      sum += sample * params.d50_gains[cfa_channel(sample_position)] * weights[kx] * weights[ky];\n    }\n  }\n  return sum / 16.0;\n}\n\n@compute @workgroup_size(1, 1, 1)\nfn lcst_average_main(@builtin(global_invocation_id) id: vec3<u32>) {\n  if (id.x >= 64u || id.y >= 48u) { return; }\n  let xb = partition_bounds(id.x, 64u, params.source_extent.x);\n  let yb = partition_bounds(id.y, 48u, params.source_extent.y);\n  var sums = vec4<f32>(0.0);\n  var counts = vec4<u32>(0u);\n  for (var y = yb.x; y < yb.y; y += 1u) {\n    for (var x = xb.x; x < xb.y; x += 1u) {\n      let channel = cfa_channel(vec2<u32>(x, y));\n      sums[channel] += textureLoad(input_texture, vec2<i32>(i32(x), i32(y)), 0).x;\n      counts[channel] += 1u;\n    }\n  }\n  average_rggb[id.y * 64u + id.x] = sums / vec4<f32>(counts);\n}\n\n@compute @workgroup_size(1, 1, 1)\nfn lcst_histogram_main(@builtin(global_invocation_id) id: vec3<u32>) {\n  if (id.x >= 16u || id.y >= 16u) { return; }\n  let xb = partition_bounds(id.x, 16u, params.source_extent.x);\n  let yb = partition_bounds(id.y, 16u, params.source_extent.y);\n  var bins = array<u32, 16>();\n  for (var y = yb.x; y < yb.y; y += 1u) {\n    for (var x = xb.x; x < xb.y; x += 1u) {\n      let luma = clamp(filtered_luma(vec2<u32>(x, y)), 0.0, 1.0);\n      let bin = min(u32(luma * 16.0), 15u);\n      bins[bin] += 1u;\n    }\n  }\n  let tile = id.y * 16u + id.x;\n  for (var bin = 0u; bin < 16u; bin += 1u) {\n    luma_histogram[tile * 16u + bin] = bins[bin];\n  }\n}\n"
} as const;
