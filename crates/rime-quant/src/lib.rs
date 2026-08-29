#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-grid `f32` quantization shared by ISP operators and shader code.

mod dither;
mod profile;
mod quantize;
pub use dither::{DitherKey, dither_u04, lfsr28_advance, lfsr28_next, rnd4b};

pub use profile::{
    ClipType, DitherProfile, QuantProfile, RimeQProfile, RoundingMode, SaturationMode,
};
pub use quantize::{QuantError, quantize_f32, quantize_f32_grid};

/// WGSL implementation of the fixed-grid quantizer.
pub const QUANTIZE_WGSL: &str = include_str!("../shaders/quantize.wgsl");
