const MASK_28: u32 = (1 << 28) - 1;

/// Advance the reference 28-bit LFSR (`x^28 + x^25 + 1`).
#[must_use]
pub fn lfsr28_next(state: u32) -> u32 {
    let state = state & MASK_28;
    ((state << 1) | (((state >> 27) ^ (state >> 24)) & 1)) & MASK_28
}

fn apply_matrix(matrix: &[u32; 28], state: u32) -> u32 {
    let mut result = 0;
    for (bit, column) in matrix.iter().enumerate() {
        if state & (1 << bit) != 0 {
            result ^= column;
        }
    }
    result & MASK_28
}

fn square_matrix(matrix: &[u32; 28]) -> [u32; 28] {
    std::array::from_fn(|bit| apply_matrix(matrix, matrix[bit]))
}

/// Advance the LFSR by `steps` positions without depending on iteration order.
#[must_use]
pub fn lfsr28_advance(mut state: u32, mut steps: u32) -> u32 {
    let mut matrix = std::array::from_fn(|bit| lfsr28_next(1 << bit));
    while steps != 0 {
        if steps & 1 != 0 {
            state = apply_matrix(&matrix, state);
        }
        matrix = square_matrix(&matrix);
        steps >>= 1;
    }
    state
}

/// Produce the reference four-bit value from a 16-bit state and key.
#[must_use]
pub fn rnd4b(rnd16: u16, key: u16) -> u8 {
    let keys = [
        u32::from((key >> 12) & 0xf),
        u32::from((key >> 8) & 0xf),
        u32::from((key >> 4) & 0xf),
        u32::from(key & 0xf),
    ];
    let mut temp = u32::from(rnd16);
    for shift in keys.iter().take(3).copied() {
        temp = ((temp << (16 - shift)) | (temp >> shift)) & 0xffff;
        temp = (temp & 0x8000)
            | ((temp & 0x4000) >> 3)
            | ((temp & 0x2000) >> 6)
            | ((temp & 0x1000) >> 9)
            | ((temp & 0x0800) << 3)
            | (temp & 0x0400)
            | ((temp & 0x0200) >> 3)
            | ((temp & 0x0100) >> 6)
            | ((temp & 0x0080) << 6)
            | ((temp & 0x0040) << 3)
            | (temp & 0x0020)
            | ((temp & 0x0010) >> 3)
            | ((temp & 0x0008) << 9)
            | ((temp & 0x0004) << 6)
            | ((temp & 0x0002) << 3)
            | (temp & 0x0001);
    }
    ((temp & 0xf) as u8) ^ u8::try_from(keys[3] & 0xf).unwrap_or(0)
}

/// Stable identity for one dither sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DitherKey {
    /// Stream seed.
    pub seed: u32,
    /// Stable quantization stream.
    pub stream_id: u32,
    /// Frame number.
    pub frame_index: u32,
    /// Plane number.
    pub plane: u32,
    /// Channel number.
    pub channel: u32,
    /// Pixel group number.
    pub pixel_group: u32,
    /// Lane within a pixels-per-call group.
    pub ppc_lane: u32,
}

/// Generate one deterministic `u04` value for a dither coordinate.
#[must_use]
pub fn dither_u04(key: DitherKey) -> f32 {
    let index = key
        .stream_id
        .wrapping_add(key.frame_index)
        .wrapping_add(key.plane)
        .wrapping_add(key.pixel_group);
    let state = lfsr28_advance(key.seed, index.wrapping_add(1));
    let lane_shift = (key.channel.saturating_mul(8) + key.ppc_lane.saturating_mul(2)).min(20);
    let rnd16 = u16::try_from((state >> lane_shift) & 0xffff).unwrap_or(0);
    let permutation_key = u16::try_from(
        ((key.channel & 0x3) << 12) | ((key.ppc_lane & 0x3) << 4) | (key.stream_id & 0xf),
    )
    .unwrap_or(0);
    f32::from(rnd4b(rnd16, permutation_key)) / 16.0
}
