use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

use crate::QuantError;

/// Quantization clipping policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipType {
    /// Truncate toward zero.
    Truncate,
    /// Compute `floor(code + 0.5)`.
    Round,
    /// Apply signed deterministic LSB dither, then truncate toward zero.
    Dither,
    /// Apply stateless GPU PRNG dither, then truncate toward zero.
    DitherGpu,
}

/// Quantization rounding policy supported by the compatibility API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingMode {
    /// Truncate toward zero.
    TruncateFloor,
    /// Compute `floor(code + 0.5)`, including for negative values.
    RoundFloorPlusHalf,
    /// Apply signed four-bit dither before truncating toward zero.
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

/// A fixed-point `Rime.Q` profile, written as `uX.Y` or `sX.Y`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RimeQProfile {
    /// Magnitude bits left of the binary point.
    pub int_bits: u8,
    /// Fractional bits right of the binary point.
    pub frac_bits: u8,
    /// Whether negative values are representable.
    pub signed: bool,
}

impl RimeQProfile {
    /// Construct and validate a fixed-point profile.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile exceeds exact `f32` carrier precision.
    pub fn new(int_bits: u8, frac_bits: u8, signed: bool) -> Result<Self, QuantError> {
        let profile = Self {
            int_bits,
            frac_bits,
            signed,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Validate that the grid is exactly representable by an `f32` carrier.
    ///
    /// # Errors
    ///
    /// Returns an error when integer and fractional bits exceed 24 total bits.
    pub fn validate(&self) -> Result<(), QuantError> {
        if u16::from(self.int_bits) + u16::from(self.frac_bits) > 24 {
            return Err(QuantError::Fp32GridPrecisionExceeded);
        }
        Ok(())
    }

    /// Binary scale `2^Y`.
    pub(crate) fn scale(self) -> f32 {
        2.0_f32.powi(i32::from(self.frac_bits))
    }

    /// Least significant bit represented by the profile.
    #[must_use]
    pub fn lsb(&self) -> f32 {
        self.scale().recip()
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
        (2.0_f32).powi(i32::from(self.int_bits)) - self.lsb()
    }
}

impl RimeQProfile {
    /// Parse a `uX.Y` or `sX.Y` notation.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed notation or profiles exceeding exact `f32` precision.
    pub fn parse(notation: &str) -> Result<Self, QuantError> {
        notation.parse()
    }

    /// Format this profile as canonical `uX.Y` or `sX.Y` notation.
    #[must_use]
    pub fn format(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for RimeQProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}.{}",
            if self.signed { 's' } else { 'u' },
            self.int_bits,
            self.frac_bits
        )
    }
}

impl FromStr for RimeQProfile {
    type Err = QuantError;

    fn from_str(notation: &str) -> Result<Self, Self::Err> {
        let (signed, body) = match notation.as_bytes().first() {
            Some(b'u') => (false, &notation[1..]),
            Some(b's') => (true, &notation[1..]),
            _ => return Err(QuantError::InvalidNotation),
        };
        let (int, frac) = body.split_once('.').ok_or(QuantError::InvalidNotation)?;
        if int.is_empty()
            || frac.is_empty()
            || !int.bytes().all(|b| b.is_ascii_digit())
            || !frac.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(QuantError::InvalidNotation);
        }
        let int_bits = int.parse::<u8>().map_err(|_| QuantError::InvalidNotation)?;
        let frac_bits = frac
            .parse::<u8>()
            .map_err(|_| QuantError::InvalidNotation)?;
        Self::new(int_bits, frac_bits, signed)
    }
}

/// Legacy profile retained as a source-compatible wrapper around `RimeQProfile`.
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
    /// Returns an error for invalid carrier precision or inconsistent dither configuration.
    pub fn validate(&self) -> Result<(), QuantError> {
        RimeQProfile::new(self.int_bits, self.frac_bits, self.signed)?;
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
