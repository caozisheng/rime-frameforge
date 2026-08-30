const MASK_28: u32 = 0x0fffffffu;
const ROUND_TRUNCATE_TOWARD_ZERO: u32 = 0u;
const ROUND_FLOOR_PLUS_HALF: u32 = 1u;
const ROUND_DITHERED: u32 = 2u;
const ROUND_DITHER_GPU: u32 = 3u;

struct QuantParams {
  scale: f32,
  qmin: f32,
  qmax: f32,
  rounding_mode: u32,
  dither_seed: u32,
  stream_id: u32,
  frame_index: u32,
  plane: u32,
  width: u32,
  height: u32,
  ppc: u32,
  channel: u32,
  groups_per_row: u32,
  groups_per_frame: u32,
  _padding0: u32,
  _padding1: u32,
}

fn lfsr28_next(state: u32) -> u32 {
  let state28 = state & MASK_28;
  return ((state28 << 1u) | (((state28 >> 27u) ^ (state28 >> 24u)) & 1u)) & MASK_28;
}

fn apply_lfsr_matrix(matrix: array<u32, 28>, state: u32) -> u32 {
  var result = 0u;
  for (var bit = 0u; bit < 28u; bit++) {
    if ((state & (1u << bit)) != 0u) { result ^= matrix[bit]; }
  }
  return result & MASK_28;
}

fn square_lfsr_matrix(matrix: array<u32, 28>) -> array<u32, 28> {
  var squared: array<u32, 28>;
  for (var bit = 0u; bit < 28u; bit++) {
    squared[bit] = apply_lfsr_matrix(matrix, matrix[bit]);
  }
  return squared;
}

fn lfsr28_advance(seed: u32, steps: u32) -> u32 {
  var state = seed & MASK_28;
  var remaining = steps;
  var matrix: array<u32, 28>;
  for (var bit = 0u; bit < 28u; bit++) {
    matrix[bit] = lfsr28_next(1u << bit);
  }
  while (remaining != 0u) {
    if ((remaining & 1u) != 0u) { state = apply_lfsr_matrix(matrix, state); }
    matrix = square_lfsr_matrix(matrix);
    remaining >>= 1u;
  }
  return state;
}

fn rnd4b(rnd16: u32, key: u32) -> u32 {
  let k0 = (key >> 12u) & 0xfu;
  let k1 = (key >> 8u) & 0xfu;
  let k2 = (key >> 4u) & 0xfu;
  let k3 = key & 0xfu;
  var value = rnd16 & 0xffffu;
  for (var shift_index = 0u; shift_index < 3u; shift_index++) {
    let shift = select(select(k2, k1, shift_index == 1u), k0, shift_index == 0u);
    value = ((value << (16u - shift)) | (value >> shift)) & 0xffffu;
    value = (value & 0x8000u)
      | ((value & 0x4000u) >> 3u) | ((value & 0x2000u) >> 6u) | ((value & 0x1000u) >> 9u)
      | ((value & 0x0800u) << 3u) | (value & 0x0400u) | ((value & 0x0200u) >> 3u) | ((value & 0x0100u) >> 6u)
      | ((value & 0x0080u) << 6u) | ((value & 0x0040u) << 3u) | (value & 0x0020u) | ((value & 0x0010u) >> 3u)
      | ((value & 0x0008u) << 9u) | ((value & 0x0004u) << 6u) | ((value & 0x0002u) << 3u) | (value & 0x0001u);
  }
  return (value & 0xfu) ^ k3;
}

fn hash_u32(input: u32) -> u32 {
  var value = input;
  value ^= value >> 16u;
  value *= 0x7feb352du;
  value ^= value >> 15u;
  value *= 0x846ca68bu;
  value ^= value >> 16u;
  return value;
}

fn gpu_random_u04(params: QuantParams, pixel_group: u32, ppc_lane: u32) -> f32 {
  var state = hash_u32(params.dither_seed ^ 0x9e3779b9u);
  state = hash_u32(state ^ hash_u32(params.stream_id + 0x243f6a88u));
  state = hash_u32(state ^ hash_u32(params.frame_index + 0xb7e15162u));
  state = hash_u32(state ^ hash_u32(params.plane + 0xdeadbeefu));
  state = hash_u32(state ^ hash_u32(params.channel + 0x85ebca6bu));
  state = hash_u32(state ^ hash_u32(pixel_group + 0xc2b2ae35u));
  state = hash_u32(state ^ hash_u32(ppc_lane + 0x27d4eb2fu));
  return f32(state & 0xfu) / 16.0;
}

fn reference_random_u04(params: QuantParams, pixel_group: u32, ppc_lane: u32) -> f32 {
  let state = lfsr28_advance(
    params.dither_seed,
    pixel_group + params.groups_per_frame * params.frame_index + 1u,
  );
  let lane_shift = min(params.channel * 8u + ppc_lane * 2u, 20u);
  let rnd16 = (state >> lane_shift) & 0xffffu;
  let key = ((params.channel & 0x3u) << 12u)
    | ((ppc_lane & 0x3u) << 4u)
    | (params.stream_id & 0xfu);
  return f32(rnd4b(rnd16, key)) / 16.0;
}

fn dither_u04_for_sample(params: QuantParams, pixel_group: u32, ppc_lane: u32) -> f32 {
  if (params.rounding_mode == ROUND_DITHER_GPU) {
    return gpu_random_u04(params, pixel_group, ppc_lane);
  }
  return reference_random_u04(params, pixel_group, ppc_lane);
}

fn truncate_toward_zero(value: f32) -> f32 {
  return select(ceil(value), floor(value), value >= 0.0);
}

fn quantize_sample(x: f32, params: QuantParams, pixel_group: u32, ppc_lane: u32) -> f32 {
  if (x == 0.0) { return 0.0; }
  var offset = 0.0;
  if (params.rounding_mode == ROUND_FLOOR_PLUS_HALF) {
    offset = 0.5;
  } else if (params.rounding_mode == ROUND_DITHERED || params.rounding_mode == ROUND_DITHER_GPU) {
    let dither = dither_u04_for_sample(params, pixel_group, ppc_lane);
    offset = select(0.5 - dither, dither - 0.5, x > 0.0);
  }
  let code_input = x * params.scale + offset;
  let code = select(floor(code_input), truncate_toward_zero(code_input),
    params.rounding_mode != ROUND_FLOOR_PLUS_HALF);
  return clamp(code / params.scale, params.qmin, params.qmax);
}

@group(0) @binding(0) var<uniform> quant_params: QuantParams;
@group(0) @binding(1) var quant_input: texture_2d<f32>;
@group(0) @binding(2) var quant_output: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn quantize_r32_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let dimensions = textureDimensions(quant_output);
  if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
  let ppc = max(quant_params.ppc, 1u);
  let pixel_group = id.y * quant_params.groups_per_row + id.x / ppc;
  let ppc_lane = id.x % ppc;
  let sample = textureLoad(quant_input, vec2<i32>(id.xy), 0).r;
  textureStore(quant_output, vec2<i32>(id.xy),
    vec4<f32>(quantize_sample(sample, quant_params, pixel_group, ppc_lane), 0.0, 0.0, 0.0));
}
