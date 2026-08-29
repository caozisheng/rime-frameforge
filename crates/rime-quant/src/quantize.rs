use thiserror::Error;

use crate::{ClipType, QuantProfile, RimeQProfile, RoundingMode, SaturationMode};

/// Errors raised by profile validation or quantization.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum QuantError {
    /// Input is NaN or infinite.
    #[error("quantization input is not finite")]
    NonFiniteInput,
    /// Dither input is NaN or infinite.
    #[error("quantization dither is not finite")]
    NonFiniteDither,
    /// Rime.Q notation is malformed.
    #[error("invalid Rime.Q notation")]
    InvalidNotation,
    /// Fixed-grid code cannot be represented exactly by an `f32` carrier.
    #[error("fixed-grid profile exceeds the exact f32 precision limit")]
    Fp32GridPrecisionExceeded,
    /// Fractional bits cannot be represented by the implementation shift.
    #[error("fractional bits are out of range")]
    FractionalBitsOutOfRange,
    /// Dithered rounding requires a dither profile.
    #[error("dithered rounding requires a dither profile")]
    MissingDitherProfile,
    /// A dither seed must be a non-zero 28-bit value.
    #[error("dither seed must be a non-zero 28-bit value")]
    InvalidDitherSeed,
    /// Non-dithered rounding must not carry a dither profile.
    #[error("dither profile is set for a non-dithered rounding mode")]
    UnexpectedDitherProfile,
    /// Saturation mode is reserved for future policies.
    #[error("unsupported saturation mode")]
    UnsupportedSaturation,
}

/// Quantize one `f32` value onto a fixed-point grid.
///
/// The arithmetic intentionally follows the `f32` reference sequence: scale,
/// apply the selected offset, floor, rescale, then saturate.
///
/// # Errors
///
/// Returns an error for invalid profiles, non-finite input, or unsupported saturation.
pub fn quantize_f32_grid(
    x: f32,
    profile: &QuantProfile,
    dither_u04: f32,
) -> Result<f32, QuantError> {
    profile.validate()?;
    if !x.is_finite() {
        return Err(QuantError::NonFiniteInput);
    }
    if !matches!(profile.saturation, SaturationMode::Clamp) {
        return Err(QuantError::UnsupportedSaturation);
    }
    let scale = profile.scale();
    let offset = match profile.rounding {
        RoundingMode::RoundFloorPlusHalf => 0.5,
        RoundingMode::Dithered if x > 0.0 => dither_u04 - 0.5,
        RoundingMode::Dithered if x < 0.0 => 0.5 - dither_u04,
        RoundingMode::TruncateFloor | RoundingMode::Dithered => 0.0,
    };
    let value = (x * scale + offset).floor() / scale;
    Ok(value.clamp(profile.qmin(), profile.qmax()))
}

/// Quantize an `f32` value using a validated Rime.Q profile and clip policy.
///
/// # Errors
///
/// Returns an error for an invalid profile or non-finite input/dither values.
pub fn quantize_f32(
    x: f32,
    profile: &RimeQProfile,
    clip: ClipType,
    dither_u04: f32,
) -> Result<f32, QuantError> {
    profile.validate()?;
    if !x.is_finite() {
        return Err(QuantError::NonFiniteInput);
    }
    if !dither_u04.is_finite() {
        return Err(QuantError::NonFiniteDither);
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    let scale = profile.scale();
    let offset = match clip {
        ClipType::Round => 0.5,
        ClipType::Dither if x > 0.0 => dither_u04 - 0.5,
        ClipType::Dither => 0.5 - dither_u04,
        ClipType::Truncate => 0.0,
    };
    let value = (x * scale + offset).floor() / scale;
    Ok(value.clamp(profile.qmin(), profile.qmax()))
}
