const MASK_28: u32 = 0x0fffffff;
const ROUND_TRUNCATE_FLOOR: u32 = 0u;
const ROUND_FLOOR_PLUS_HALF: u32 = 1u;
const ROUND_DITHERED: u32 = 2u;

struct QuantParams {
  scale: f32,
  qmin: f32,
  qmax: f32,
  rounding_mode: u32,
}

fn lfsr28_next(state: u32) -> u32 {
  let state28 = state & MASK_28;
  return ((state28 << 1u) | (((state28 >> 27u) ^ (state28 >> 24u)) & 1u)) & MASK_28;
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

fn quantize_f32_grid(x: f32, params: QuantParams, dither_u04: f32) -> f32 {
  var offset = 0.0;
  if (params.rounding_mode == ROUND_FLOOR_PLUS_HALF) {
    offset = 0.5;
  } else if (params.rounding_mode == ROUND_DITHERED) {
    if (x > 0.0) {
      offset = dither_u04 - 0.5;
    } else if (x < 0.0) {
      offset = 0.5 - dither_u04;
    }
  }
  let code = floor(x * params.scale + offset);
  return clamp(code / params.scale, params.qmin, params.qmax);
}
