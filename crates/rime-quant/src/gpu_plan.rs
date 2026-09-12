//! GPU quantization plan derivation — the Rust authority for the
//! `QuantParams` WGSL block (`shaders/quantize.wgsl`).
//!
//! Every consumer embedding `QuantParams` (the fused super-uniform, future
//! native port quantizers) derives field values and byte layout from here;
//! the WGSL field order is pinned to this module by test.

use serde::{Deserialize, Serialize};

use crate::QuantError;
use crate::profile::{ClipType, RimeQProfile};

/// WGSL `QuantParams` block size in bytes (16 × `u32`).
pub const QUANT_PARAMS_BYTES: usize = 64;

/// Default 28-bit LFSR seed shared by every GPU quantization stream.
pub const DEFAULT_SEED: u32 = 0x1a5b_6cfd;

/// Pixels per quantization group for GPU streams.
pub const DEFAULT_PPC: u32 = 1;

/// Field-order pin for the WGSL `QuantParams` struct — used only by the
/// parity test; the serializer itself is field-position driven.
#[cfg(test)]
const WGSL_FIELD_ORDER: [&str; 16] = [
    "scale",
    "qmin",
    "qmax",
    "rounding_mode",
    "dither_seed",
    "stream_id",
    "frame_index",
    "plane",
    "width",
    "height",
    "ppc",
    "channel",
    "groups_per_row",
    "groups_per_frame",
    "_padding0",
    "_padding1",
];
/// One module output port's derived GPU quantization plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuQuantPlan {
    /// Fixed-point `Rime.Q` profile for the port.
    pub profile: RimeQProfile,
    /// Whether quantization executes for this port this frame
    /// (graph AND module AND presentation enabled; not part of the
    /// `QuantParams` block — the fused uniform carries separate flags).
    pub output_enabled: bool,
    /// WGSL rounding-mode selector (`0..=3`).
    pub rounding_mode: u32,
    /// 28-bit LFSR dither seed.
    pub seed: u32,
    /// Stable stream identifier, unique per module port.
    pub stream_id: u32,
    /// Frame index fed to the dither stream.
    pub frame_index: u32,
    /// Plane selector (single-plane outputs use `0`).
    pub plane: u32,
    /// Pixels per quantization group.
    pub ppc: u32,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Quantization groups per row (`ceil(width / ppc)`).
    pub groups_per_row: u32,
    /// Quantization groups per frame.
    pub groups_per_frame: u32,
}

/// Derivation inputs for one module output port's plan.
#[derive(Clone, Copy, Debug)]
pub struct GpuQuantPlanRequest {
    /// Fixed-point `Rime.Q` profile for the port.
    pub profile: RimeQProfile,
    /// Clipping policy.
    pub clip_type: ClipType,
    /// Whether quantization executes for this port this frame.
    pub output_enabled: bool,
    /// Stable stream identifier, unique per module port.
    pub stream_id: u32,
    /// Frame index fed to the dither stream.
    pub frame_index: u32,
    /// Plane selector (single-plane outputs use `0`).
    pub plane: u32,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

impl GpuQuantPlan {
    /// Derive the plan for one module output port.
    ///
    /// # Errors
    ///
    /// Returns [`QuantError::Fp32GridPrecisionExceeded`] when the profile
    /// exceeds exact `f32` carrier precision.
    ///
    /// # Panics
    ///
    /// Never panics; group counts saturate instead of overflowing.
    pub fn derive(request: GpuQuantPlanRequest) -> Result<Self, QuantError> {
        request.profile.validate()?;
        let groups_per_row = request.width.div_ceil(DEFAULT_PPC);
        Ok(Self {
            profile: request.profile,
            output_enabled: request.output_enabled,
            rounding_mode: rounding_mode_u32(request.clip_type),
            seed: DEFAULT_SEED,
            stream_id: request.stream_id,
            frame_index: request.frame_index,
            plane: request.plane,
            ppc: DEFAULT_PPC,
            width: request.width,
            height: request.height,
            groups_per_row,
            groups_per_frame: groups_per_row.saturating_mul(request.height),
        })
    }
    /// Binary scale `2^Y` of the profile.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.profile.scale()
    }

    /// Smallest representable physical value.
    #[must_use]
    pub fn qmin(&self) -> f32 {
        self.profile.qmin()
    }

    /// Largest representable physical value.
    #[must_use]
    pub fn qmax(&self) -> f32 {
        self.profile.qmax()
    }

    /// Serialize to the fixed 64-byte `QuantParams` WGSL block
    /// (little-endian words, field order per [`WGSL_FIELD_ORDER`]).
    ///
    /// `channel` is left at `0`: single-sample ports quantize one channel,
    /// and RGBA ports override it per component at the call site.
    #[must_use]
    pub fn to_wgsl_bytes(&self) -> [u8; QUANT_PARAMS_BYTES] {
        let words = [
            self.scale().to_bits(),
            self.qmin().to_bits(),
            self.qmax().to_bits(),
            self.rounding_mode,
            self.seed,
            self.stream_id,
            self.frame_index,
            self.plane,
            self.width,
            self.height,
            self.ppc,
            0,
            self.groups_per_row,
            self.groups_per_frame,
            0,
            0,
        ];
        let mut bytes = [0_u8; QUANT_PARAMS_BYTES];
        for (index, word) in words.into_iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

/// Map a clipping policy to the WGSL rounding-mode selector.
#[must_use]
pub fn rounding_mode_u32(clip_type: ClipType) -> u32 {
    match clip_type {
        ClipType::Truncate => 0,
        ClipType::Round => 1,
        ClipType::Dither => 2,
        ClipType::DitherGpu => 3,
    }
}

/// One module output port's quantization preference — the graph-config
/// shape deserialized from user preferences (`options_json` on the wasm
/// derivation face).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GpuQuantModuleConfig {
    /// Module id the preference addresses (canonical graph address).
    pub module_id: String,
    /// Whether the user enabled quantization for this port.
    pub output_enabled: bool,
    /// Output profile in `uX.Y` / `sX.Y` notation.
    pub output_profile: String,
    /// Clipping policy.
    pub clip_type: ClipType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn f32_at(bytes: &[u8], index: usize) -> f32 {
        f32::from_ne_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("4-byte slice"),
        )
    }

    fn u32_at(bytes: &[u8], index: usize) -> u32 {
        u32::from_ne_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("4-byte slice"),
        )
    }
    #[test]
    fn derives_unsigned_profile_fields() {
        let profile = RimeQProfile::from_str("u0.14").expect("valid profile");
        let plan = GpuQuantPlan::derive(GpuQuantPlanRequest {
            profile,
            clip_type: ClipType::Truncate,
            output_enabled: true,
            stream_id: 1,
            frame_index: 7,
            plane: 0,
            width: 3744,
            height: 2776,
        })
        .expect("valid plan");
        assert!((plan.scale() - 16_384.0).abs() < f32::EPSILON);
        assert!((plan.qmin() - 0.0).abs() < f32::EPSILON);
        assert!((plan.qmax() - (1.0 - 1.0 / 16_384.0)).abs() < 1e-9);
        assert_eq!(plan.rounding_mode, 0);
        assert_eq!(plan.seed, DEFAULT_SEED);
        assert_eq!(plan.stream_id, 1);
        assert_eq!(plan.groups_per_row, 3744);
        assert_eq!(plan.groups_per_frame, 3744 * 2776);
    }

    #[test]
    fn derives_signed_profile_fields() {
        let profile = RimeQProfile::from_str("s2.12").expect("valid profile");
        let plan = GpuQuantPlan::derive(GpuQuantPlanRequest {
            profile,
            clip_type: ClipType::DitherGpu,
            output_enabled: false,
            stream_id: 3,
            frame_index: 0,
            plane: 0,
            width: 64,
            height: 8,
        })
        .expect("valid plan");
        assert!((plan.qmin() - (-4.0)).abs() < f32::EPSILON);
        assert!((plan.qmax() - (4.0 - 1.0 / 4096.0)).abs() < 1e-6);
        assert_eq!(plan.rounding_mode, 3);
        assert!(!plan.output_enabled);
    }

    #[test]
    fn rejects_precision_overflow() {
        // `RimeQProfile::new` rejects this at construction; build the
        // invalid shape directly so `derive` must catch it via `validate`.
        let profile = RimeQProfile {
            int_bits: 20,
            frac_bits: 10,
            signed: false,
        };
        assert!(
            GpuQuantPlan::derive(GpuQuantPlanRequest {
                profile,
                clip_type: ClipType::Round,
                output_enabled: true,
                stream_id: 1,
                frame_index: 0,
                plane: 0,
                width: 8,
                height: 8,
            })
            .is_err()
        );
    }
    #[test]
    fn maps_every_rounding_mode() {
        assert_eq!(rounding_mode_u32(ClipType::Truncate), 0);
        assert_eq!(rounding_mode_u32(ClipType::Round), 1);
        assert_eq!(rounding_mode_u32(ClipType::Dither), 2);
        assert_eq!(rounding_mode_u32(ClipType::DitherGpu), 3);
    }
    #[test]
    fn serializes_the_wgsl_field_order() {
        let profile = RimeQProfile::from_str("u0.14").expect("valid profile");
        let plan = GpuQuantPlan::derive(GpuQuantPlanRequest {
            profile,
            clip_type: ClipType::Dither,
            output_enabled: true,
            stream_id: 6,
            frame_index: 42,
            plane: 0,
            width: 3744,
            height: 2776,
        })
        .expect("valid plan");
        let bytes = plan.to_wgsl_bytes();
        assert_eq!(bytes.len(), QUANT_PARAMS_BYTES);
        assert!((f32_at(&bytes, 0) - plan.scale()).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 1) - plan.qmin()).abs() < f32::EPSILON);
        assert!((f32_at(&bytes, 2) - plan.qmax()).abs() < 1e-9);
        assert_eq!(u32_at(&bytes, 3), 2);
        assert_eq!(u32_at(&bytes, 4), DEFAULT_SEED);
        assert_eq!(u32_at(&bytes, 5), 6);
        assert_eq!(u32_at(&bytes, 6), 42);
        assert_eq!(u32_at(&bytes, 7), 0);
        assert_eq!(u32_at(&bytes, 8), 3744);
        assert_eq!(u32_at(&bytes, 9), 2776);
        assert_eq!(u32_at(&bytes, 10), DEFAULT_PPC);
        assert_eq!(u32_at(&bytes, 11), 0);
        assert_eq!(u32_at(&bytes, 12), 3744);
        assert_eq!(u32_at(&bytes, 13), 3744 * 2776);
        assert_eq!(u32_at(&bytes, 14), 0);
        assert_eq!(u32_at(&bytes, 15), 0);
    }

    /// Single-source pin: the WGSL struct this plan serializes for must
    /// declare exactly the fields [`GpuQuantPlan::to_wgsl_bytes`] writes,
    /// in the same word order.
    #[test]
    fn wgsl_struct_matches_serialization_order() {
        let source = crate::QUANTIZE_WGSL;
        let start = source
            .find("struct QuantParams {")
            .expect("QuantParams struct present");
        let body = &source[start..];
        let end = body.find('}').expect("struct terminator");
        let fields = body[..end]
            .trim_start_matches("struct QuantParams {")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.trim_end_matches(','))
            .map(|line| line.split(':').next().unwrap_or_default().trim())
            .collect::<Vec<_>>();
        assert_eq!(
            fields, WGSL_FIELD_ORDER,
            "QuantParams drifted from the Rust serializer"
        );
    }
}
