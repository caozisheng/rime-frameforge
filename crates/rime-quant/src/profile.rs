use crate::QuantError;

/// Quantization rounding policy supported by both Rust and WGSL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingMode {
    /// Round toward negative infinity.
    TruncateFloor,
    /// Compute `floor(code + 0.5)`, including for negative values.
    RoundFloorPlusHalf,
    /// Apply signed four-bit dither before rounding toward negative infinity.
    Dithered,
}

/// Overflow policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaturationMode {
    /// Clamp to the representable Q-format range.
    Clamp,
}

/// Configuration for deterministic four-bit dither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DitherProfile {
    /// Stable random stream identifier.
    pub stream_id: u32,
    /// 28-bit LFSR seed. Zero is invalid.
    pub seed: u32,
    /// Sixteen-bit reference permutation key.
    pub key: u16,
}

impl Default for DitherProfile {
    fn default() -> Self {
        Self {
            stream_id: 0,
            seed: 1,
            key: 0,
        }
    }
}

/// Fixed-point precision and rounding contract carried in `f32` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantProfile {
    /// Magnitude bits left of the binary point; excludes the sign bit.
    pub int_bits: u8,
    /// Bits right of the binary point.
    pub frac_bits: u8,
    /// Whether negative values are representable.
    pub signed: bool,
    /// Rounding policy.
    pub rounding: RoundingMode,
    /// Overflow policy.
    pub saturation: SaturationMode,
    /// Dither stream configuration, required only for dithered rounding.
    pub dither: Option<DitherProfile>,
}

impl QuantProfile {
    /// Validate exact `f32` carrier and dither invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile exceeds carrier precision or has an invalid dither configuration.
    pub fn validate(&self) -> Result<(), QuantError> {
        if u16::from(self.int_bits) + u16::from(self.frac_bits) > 24 {
            return Err(QuantError::Fp32GridPrecisionExceeded);
        }
        if self.frac_bits >= 32 {
            return Err(QuantError::FractionalBitsOutOfRange);
        }
        match (self.rounding, self.dither) {
            (RoundingMode::Dithered, None) => Err(QuantError::MissingDitherProfile),
            (RoundingMode::Dithered, Some(dither))
                if dither.seed == 0 || dither.seed >= (1 << 28) =>
            {
                Err(QuantError::InvalidDitherSeed)
            }
            (RoundingMode::TruncateFloor | RoundingMode::RoundFloorPlusHalf, Some(_)) => {
                Err(QuantError::UnexpectedDitherProfile)
            }
            _ => Ok(()),
        }
    }

    /// Binary scale `2^F`.
    pub(crate) fn scale(&self) -> f32 {
        2.0_f32.powi(i32::from(self.frac_bits))
    }

    /// Smallest representable physical value.
    #[must_use]
    pub fn qmin(&self) -> f32 {
        if self.signed {
            -(2.0_f32).powi(i32::from(self.int_bits))
        } else {
            0.0
        }
    }

    /// Largest representable physical value.
    #[must_use]
    pub fn qmax(&self) -> f32 {
        (2.0_f32).powi(i32::from(self.int_bits)) - self.scale().recip()
    }
}
