#![allow(clippy::float_cmp)]

use rime_quant::{
    DitherKey, DitherProfile, QuantError, QuantProfile, RoundingMode, SaturationMode, dither_u04,
    lfsr28_next, quantize_f32_grid, rnd4b,
};

fn profile(rounding: RoundingMode, signed: bool) -> QuantProfile {
    QuantProfile {
        int_bits: 1,
        frac_bits: 2,
        signed,
        rounding,
        saturation: SaturationMode::Clamp,
        dither: None,
    }
}

#[test]
fn truncate_floor_matches_reference_for_negative_values() {
    let p = profile(RoundingMode::TruncateFloor, true);
    assert_eq!(quantize_f32_grid(-0.26, &p, 0.0), Ok(-0.5));
    assert_eq!(quantize_f32_grid(0.26, &p, 0.0), Ok(0.25));
}

#[test]
fn round_floor_plus_half_uses_floor_not_standard_rounding() {
    let p = profile(RoundingMode::RoundFloorPlusHalf, true);
    assert_eq!(quantize_f32_grid(-0.375, &p, 0.0), Ok(-0.25));
    assert_eq!(quantize_f32_grid(0.375, &p, 0.0), Ok(0.5));
}

#[test]
fn quantization_saturates_after_rounding() {
    let p = profile(RoundingMode::RoundFloorPlusHalf, true);
    assert_eq!(quantize_f32_grid(1.99, &p, 0.0), Ok(1.75));
    assert_eq!(quantize_f32_grid(-2.1, &p, 0.0), Ok(-2.0));
}

#[test]
fn non_finite_values_are_rejected() {
    let p = profile(RoundingMode::TruncateFloor, true);
    assert_eq!(
        quantize_f32_grid(f32::NAN, &p, 0.0),
        Err(QuantError::NonFiniteInput)
    );
    assert_eq!(
        quantize_f32_grid(f32::INFINITY, &p, 0.0),
        Err(QuantError::NonFiniteInput)
    );
}

#[test]
fn profile_rejects_non_exact_fp32_grid() {
    let p = QuantProfile {
        int_bits: 9,
        frac_bits: 16,
        signed: true,
        rounding: RoundingMode::TruncateFloor,
        saturation: SaturationMode::Clamp,
        dither: None,
    };
    assert_eq!(p.validate(), Err(QuantError::Fp32GridPrecisionExceeded));
}

#[test]
fn dither_zero_does_not_inject_signal() {
    let p = QuantProfile {
        int_bits: 1,
        frac_bits: 4,
        signed: true,
        rounding: RoundingMode::Dithered,
        saturation: SaturationMode::Clamp,
        dither: Some(DitherProfile::default()),
    };
    assert_eq!(quantize_f32_grid(0.0, &p, 0.9375), Ok(0.0));
}

#[test]
fn high_fractional_precision_keeps_exact_grid_scale() {
    let p = QuantProfile {
        int_bits: 1,
        frac_bits: 16,
        signed: true,
        rounding: RoundingMode::TruncateFloor,
        saturation: SaturationMode::Clamp,
        dither: None,
    };
    assert_eq!(quantize_f32_grid(1.0, &p, 0.0), Ok(1.0));
    assert_eq!(quantize_f32_grid(0.00002, &p, 0.0), Ok(0.000_015_258_789));
}
#[test]
fn lfsr_and_rnd4b_are_deterministic() {
    assert_eq!(lfsr28_next(0x1a5b_6cfd), 0x04b6_d9fb);
    assert_eq!(rnd4b(0x1234, 0x13ab), 0x01);
    let key = DitherKey {
        seed: 0x1a5b_6cfd,
        stream_id: 3,
        frame_index: 4,
        plane: 0,
        channel: 1,
        pixel_group: 8,
        ppc_lane: 0,
    };
    assert_eq!(dither_u04(key), dither_u04(key));
}
