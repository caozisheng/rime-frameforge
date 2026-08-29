#![allow(clippy::float_cmp)]

use std::str::FromStr;

use rime_quant::{
    ClipType, DitherKey, DitherProfile, QuantError, QuantProfile, RimeQProfile, RoundingMode,
    SaturationMode, dither_u04, lfsr28_next, quantize_f32, quantize_f32_grid, rnd4b,
};

#[test]
fn rime_q_notation_round_trips_and_exposes_range() {
    let unsigned = RimeQProfile::from_str("u0.14").unwrap();
    assert_eq!(unsigned.to_string(), "u0.14");
    assert_eq!(unsigned.lsb(), 1.0 / 16384.0);
    assert_eq!(unsigned.qmin(), 0.0);
    assert_eq!(unsigned.qmax(), 1.0 - unsigned.lsb());

    let signed = RimeQProfile::from_str("s1.12").unwrap();
    assert_eq!(signed.to_string(), "s1.12");
    assert_eq!(signed.qmin(), -2.0);
    assert_eq!(signed.qmax(), 2.0 - signed.lsb());
}

#[test]
fn rime_q_notation_rejects_invalid_and_inexact_bounds() {
    for notation in ["", "x1.2", "u.14", "u1", "u1.2.3", "u-1.2", "U1.2"] {
        assert!(RimeQProfile::from_str(notation).is_err(), "{notation}");
    }
    assert!(RimeQProfile::from_str("u9.16").is_err());
    assert!(RimeQProfile::new(9, 16, false).is_err());
    assert!(RimeQProfile::new(0, 25, false).is_err());
}

#[test]
fn clip_types_follow_floor_round_and_signed_lsb_dither() {
    let profile = RimeQProfile::new(1, 2, true).unwrap();
    assert_eq!(
        quantize_f32(-0.26, &profile, ClipType::Truncate, 0.0),
        Ok(-0.5)
    );
    assert_eq!(
        quantize_f32(0.26, &profile, ClipType::Truncate, 0.0),
        Ok(0.25)
    );
    assert_eq!(
        quantize_f32(-0.375, &profile, ClipType::Round, 0.0),
        Ok(-0.25)
    );
    assert_eq!(quantize_f32(0.375, &profile, ClipType::Round, 0.0), Ok(0.5));
    assert_eq!(
        quantize_f32(0.26, &profile, ClipType::Dither, 1.0),
        Ok(0.25)
    );
    assert_eq!(
        quantize_f32(-0.26, &profile, ClipType::Dither, 0.0),
        Ok(-0.25)
    );
}

#[test]
fn clip_types_saturate_and_reject_non_finite_values() {
    let unsigned = RimeQProfile::from_str("u0.14").unwrap();
    assert_eq!(
        quantize_f32(-1.0, &unsigned, ClipType::Truncate, 0.0),
        Ok(0.0)
    );
    assert_eq!(
        quantize_f32(2.0, &unsigned, ClipType::Round, 0.0),
        Ok(unsigned.qmax())
    );
    assert_eq!(
        quantize_f32(f32::NAN, &unsigned, ClipType::Truncate, 0.0),
        Err(QuantError::NonFiniteInput)
    );
    assert_eq!(
        quantize_f32(f32::INFINITY, &unsigned, ClipType::Round, 0.0),
        Err(QuantError::NonFiniteInput)
    );
    assert_eq!(
        quantize_f32(0.5, &unsigned, ClipType::Dither, f32::NAN),
        Err(QuantError::NonFiniteDither)
    );
}

#[test]
fn dither_is_deterministic_lsb_bounded_and_zero_safe() {
    let profile = RimeQProfile::from_str("s1.12").unwrap();
    let a = quantize_f32(0.12345, &profile, ClipType::Dither, 0.0).unwrap();
    let b = quantize_f32(0.12345, &profile, ClipType::Dither, 1.0).unwrap();
    assert_eq!(
        a,
        quantize_f32(0.12345, &profile, ClipType::Dither, 0.0).unwrap()
    );
    assert!((a - b).abs() <= profile.lsb());
    assert_eq!(quantize_f32(0.0, &profile, ClipType::Dither, 1.0), Ok(0.0));
}

#[test]
fn exact_fp32_grid_is_available_through_new_profile() {
    let profile = RimeQProfile::new(1, 23, true).unwrap();
    assert_eq!(profile.lsb(), 2.0_f32.powi(-23));
    assert_eq!(
        quantize_f32(1.0, &profile, ClipType::Truncate, 0.0),
        Ok(1.0)
    );
    assert!(RimeQProfile::new(2, 23, true).is_err());
}

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
