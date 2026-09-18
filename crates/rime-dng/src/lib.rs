#![forbid(unsafe_code)]

use std::borrow::Cow;

use std::path::{Path, PathBuf};

use gamut_dng::{DngDecoder, OpcodeList, RawPhotometry, Value, cfa_color, opcode_id, tags};
use gamut_ifd::{IfdReader, RawIfd, ReadAt, Variant};
use sha2::{Digest, Sha256};
use thiserror::Error;

const BRIGHTNESS_VALUE_TAG: u16 = 37379;
const EXPOSURE_BIAS_VALUE_TAG: u16 = 37380;

fn exif_scalar(tags: &[gamut_dng::RawTag], tag: u16) -> Option<f64> {
    let value = tags.iter().find(|raw| raw.tag == tag)?.value.as_rationals();
    if let Some(rationals) = value {
        return rational_scalar(rationals.first().copied());
    }
    let signed = tags
        .iter()
        .find(|raw| raw.tag == tag)?
        .value
        .as_srationals()?;
    signed_scalar(signed.first().copied())
}

fn rational_scalar(value: Option<(u32, u32)>) -> Option<f64> {
    let (numerator, denominator) = value?;
    (denominator != 0).then_some(f64::from(numerator) / f64::from(denominator))
}

fn signed_scalar(value: Option<(i32, i32)>) -> Option<f64> {
    let (numerator, denominator) = value?;
    (denominator != 0).then_some(f64::from(numerator) / f64::from(denominator))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BayerCfa {
    Rggb,
    Grbg,
    Gbrg,
    Bggr,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawFrameLayout {
    pub width: u32,
    pub height: u32,
    pub row_stride_samples: u32,
    pub storage_bits: u8,
    pub cfa: BayerCfa,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DngRawTag {
    pub tag: u16,
    pub field_type: String,
    pub count: u64,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DngOpcode {
    pub id: u32,
    pub spec_version: [u8; 4],
    pub flags: u32,
    pub parameters: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DngOpcodeList {
    pub opcodes: Vec<DngOpcode>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpRectilinearCoefficientSet {
    pub radial: [f64; 4],
    pub tangential: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct WarpRectilinearOpcode {
    pub spec_version: [u8; 4],
    pub flags: u32,
    pub coefficient_sets: Vec<WarpRectilinearCoefficientSet>,
    pub optical_center: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixVignetteRadialOpcode {
    pub spec_version: [u8; 4],
    pub flags: u32,
    pub coefficients: [f64; 5],
    pub optical_center: [f64; 2],
}

impl FixVignetteRadialOpcode {
    /// Returns whether preview-quality rendering may skip this opcode.
    #[must_use]
    pub const fn skip_for_preview(self) -> bool {
        self.flags & gamut_dng::Opcode::FLAG_PREVIEW_SKIP != 0
    }
}

/// Typed DNG `GainMap` opcode (opcode id 9) from `OpcodeList2` — a spatially
/// varying gain mesh, in normalized pixel coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct GainMapOpcode {
    pub spec_version: [u8; 4],
    pub flags: u32,
    /// Top, left, bottom, right bounds in pixels, exclusive at bottom/right.
    pub area: [i32; 4],
    /// First plane index the map applies to.
    pub first_plane: u32,
    /// Number of planes the map applies to (at least 1).
    pub plane_count: u32,
    /// Row and column pitch of the application grid.
    pub row_pitch: u32,
    pub col_pitch: u32,
    /// Mesh size, [vertical, horizontal] entries.
    pub points: [u32; 2],
    /// Mesh grid spacing in normalized image coordinates.
    pub spacing: [f64; 2],
    /// Mesh grid origin in normalized image coordinates.
    pub origin: [f64; 2],
    /// Number of planes in the mesh data.
    pub map_planes: u32,
    /// Mesh entries, `[row][col][plane]` order, `points.v * points.h * map_planes` values.
    pub entries: Vec<f32>,
}

impl GainMapOpcode {
    /// Returns whether preview-quality rendering may skip this opcode.
    #[must_use]
    pub fn skip_for_preview(&self) -> bool {
        self.flags & gamut_dng::Opcode::FLAG_PREVIEW_SKIP != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DngMetadata {
    pub dng_version: [u8; 4],
    pub backward_version: Option<[u8; 4]>,
    pub black_repeat: (u16, u16),
    pub black_levels: Vec<f64>,
    pub black_delta_h: Option<Vec<f64>>,
    pub black_delta_v: Option<Vec<f64>>,
    pub white_levels: Vec<f64>,
    pub linearization_table: Option<Vec<u16>>,
    pub camera_model: String,
    pub color_matrix1: [f64; 9],
    pub calibration_illuminant1: String,
    pub calibration_illuminant1_code: Option<u16>,
    pub calibration_illuminant2_code: Option<u16>,
    pub camera_calibration_signature: Option<String>,
    pub profile_calibration_signature: Option<String>,
    pub profile_hue_sat_map_dims: Option<[u32; 3]>,
    pub profile_hue_sat_map_data1: Option<Vec<f32>>,
    pub profile_hue_sat_map_data2: Option<Vec<f32>>,
    pub as_shot_neutral: Option<[f64; 3]>,
    pub as_shot_white_xy: Option<[f64; 2]>,
    pub color_matrix2: Option<[f64; 9]>,
    pub camera_calibration1: Option<[f64; 9]>,
    pub camera_calibration2: Option<[f64; 9]>,
    pub forward_matrix1: Option<[f64; 9]>,
    pub forward_matrix2: Option<[f64; 9]>,
    pub analog_balance: Option<[f64; 3]>,
    pub baseline_exposure: Option<f64>,
    pub profile_name: Option<String>,
    pub exif_exposure_time: Option<(u32, u32)>,
    pub exif_f_number: Option<(u32, u32)>,
    pub exif_iso_speed: Option<u16>,
    pub exif_brightness_value: Option<f64>,
    pub exif_exposure_bias_value: Option<f64>,
    pub exif_date_time_original: Option<String>,
    pub exif_focal_length: Option<(u32, u32)>,
    pub xmp_byte_length: Option<usize>,
    pub iptc_byte_length: Option<usize>,
    pub icc_byte_length: Option<usize>,
    pub new_raw_image_digest: Option<String>,
    pub ifd0_extra: Vec<DngRawTag>,
    pub raw_extra: Vec<DngRawTag>,
    pub exif_extra: Vec<DngRawTag>,
    pub warp_rectilinear: Vec<WarpRectilinearOpcode>,
    pub fix_vignette_radial: Vec<FixVignetteRadialOpcode>,
    pub opcode_lists: [DngOpcodeList; 3],
    pub gain_map: Vec<GainMapOpcode>,
    pub metadata_hash: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedRawFrame {
    pub frame_index: u64,
    pub raw: gamut_dng::RawImage,
    pub layout: RawFrameLayout,
    pub metadata: DngMetadata,
    pub raw_digest: String,
}

impl DecodedRawFrame {
    #[must_use]
    pub fn samples(&self) -> &[u16] {
        self.raw.samples()
    }

    #[must_use]
    pub fn sample_bytes_le(&self) -> Cow<'_, [u8]> {
        #[cfg(target_endian = "little")]
        {
            Cow::Borrowed(bytemuck::cast_slice(self.samples()))
        }
        #[cfg(target_endian = "big")]
        {
            Cow::Owned(
                self.samples()
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect(),
            )
        }
    }
}

#[derive(Debug, Error)]
pub enum DngReaderError {
    #[error("failed to read DNG {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("DNG decoder rejected {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("DNG version {0:?} is below 1.4.0.0")]
    UnsupportedVersion([u8; 4]),
    #[error("DNG is not a Bayer CFA image")]
    UnsupportedPhotometry,
    #[error("DNG CFA pattern is not a supported 2x2 Bayer layout")]
    UnsupportedCfa,
    #[error("DNG sample format is not an unsigned integer")]
    UnsupportedSampleFormat,
    #[error("DNG storage bit depth {0} is outside 1..=16")]
    UnsupportedBitDepth(u16),
    #[error("DNG decoded sample count does not match its dimensions")]
    SampleCountMismatch,
    #[error("DNG camera profile is missing required calibration data")]
    MissingCalibration,
    #[error("invalid WarpRectilinear opcode at OpcodeList3 index {opcode_index}: {reason}")]
    InvalidWarpRectilinear {
        opcode_index: usize,
        reason: &'static str,
    },

    #[error("invalid GainMap opcode at OpcodeList2 index {opcode_index}: {reason}")]
    InvalidGainMap {
        opcode_index: usize,
        reason: &'static str,
    },
    #[error("invalid FixVignetteRadial opcode at OpcodeList2 index {opcode_index}: {reason}")]
    InvalidFixVignetteRadial {
        opcode_index: usize,
        reason: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DngReader;

impl DngReader {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Decodes one DNG file into the project-owned RAW frame contract.
    ///
    /// # Errors
    ///
    /// Returns an I/O, decoder, version, photometry, sample, or calibration error.
    pub fn decode_file(
        &self,
        path: &Path,
        frame_index: u64,
    ) -> Result<DecodedRawFrame, DngReaderError> {
        let data = std::fs::read(path).map_err(|source| DngReaderError::Io {
            path: path.to_owned(),
            source,
        })?;
        self.decode_bytes(path, &data, frame_index)
    }

    /// Decodes DNG bytes into the project-owned RAW frame contract.
    ///
    /// # Errors
    ///
    /// Returns a decoder, version, photometry, sample, or calibration error.
    pub fn decode_bytes(
        &self,
        path: &Path,
        data: &[u8],
        frame_index: u64,
    ) -> Result<DecodedRawFrame, DngReaderError> {
        let source_white_balance =
            source_white_balance(data).map_err(|error| DngReaderError::Decode {
                path: path.to_owned(),
                message: error.to_string(),
            })?;
        let color_calibration =
            color_calibration(data).map_err(|error| DngReaderError::Decode {
                path: path.to_owned(),
                message: error.to_string(),
            })?;
        let decode_data = decoder_compatible_data(data, source_white_balance).map_err(|error| {
            DngReaderError::Decode {
                path: path.to_owned(),
                message: error.to_string(),
            }
        })?;
        let decoded =
            DngDecoder::new()
                .decode(&decode_data)
                .map_err(|error| DngReaderError::Decode {
                    path: path.to_owned(),
                    message: error.to_string(),
                })?;
        Self::validate_version(decoded.dng_version)?;
        let raw = &decoded.raw;
        let width = raw.dimensions().width;
        let height = raw.dimensions().height;
        let cfa = bayer_cfa(raw.photometry())?;
        let storage_bits = raw.bits_per_sample();
        if !(1..=16).contains(&storage_bits) {
            return Err(DngReaderError::UnsupportedBitDepth(storage_bits));
        }
        if raw.samples_per_pixel() != 1 {
            return Err(DngReaderError::UnsupportedPhotometry);
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or(DngReaderError::SampleCountMismatch)?;
        if raw.samples().len() != expected {
            return Err(DngReaderError::SampleCountMismatch);
        }
        if decoded
            .profile
            .color_matrix1()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(DngReaderError::MissingCalibration);
        }
        let raw_digest = digest_u16(raw.samples());
        let opcode_lists = raw_opcode_lists(raw);
        let typed_opcodes = TypedOpcodes {
            warp_rectilinear: parse_warp_rectilinear_opcodes(raw.opcode_list3())?,
            fix_vignette_radial: parse_fix_vignette_radial_opcodes(raw.opcode_list2())?,
            gain_map: parse_gain_map_opcodes(raw.opcode_list2())?,
        };
        let metadata = metadata_from_decoded(
            &decoded,
            raw,
            source_white_balance,
            color_calibration,
            typed_opcodes,
            opcode_lists,
        );
        let storage_bits = u8::try_from(storage_bits)
            .map_err(|_| DngReaderError::UnsupportedBitDepth(storage_bits))?;
        Ok(DecodedRawFrame {
            frame_index,
            raw: decoded.raw,
            raw_digest,
            layout: RawFrameLayout {
                width,
                height,
                row_stride_samples: width,
                storage_bits,
                cfa,
            },
            metadata,
        })
    }

    /// Validates the supported DNG version range.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedVersion` below DNG 1.4.0.0.
    pub fn validate_version(version: [u8; 4]) -> Result<(), DngReaderError> {
        if version < [1, 4, 0, 0] {
            return Err(DngReaderError::UnsupportedVersion(version));
        }
        Ok(())
    }

    /// Validates a decoded RAW layout before GPU upload.
    ///
    /// # Errors
    ///
    /// Returns a stable error for unsupported CFA, bit depth, stride, or dimensions.
    pub fn validate_layout(layout: &RawFrameLayout) -> Result<(), DngReaderError> {
        if layout.cfa == BayerCfa::Unsupported {
            return Err(DngReaderError::UnsupportedPhotometry);
        }
        if !(1..=16).contains(&layout.storage_bits) {
            return Err(DngReaderError::UnsupportedBitDepth(
                layout.storage_bits.into(),
            ));
        }
        let expected = (layout.row_stride_samples as usize)
            .checked_mul(layout.height as usize)
            .ok_or(DngReaderError::SampleCountMismatch)?;
        if expected == 0 || layout.width > layout.row_stride_samples || layout.height == 0 {
            return Err(DngReaderError::SampleCountMismatch);
        }
        Ok(())
    }
}

fn bayer_cfa(photometry: &RawPhotometry) -> Result<BayerCfa, DngReaderError> {
    let RawPhotometry::Cfa {
        repeat,
        pattern,
        plane_color,
        ..
    } = photometry
    else {
        return Err(DngReaderError::UnsupportedPhotometry);
    };
    if *repeat != (2, 2)
        || plane_color.as_slice() != [cfa_color::RED, cfa_color::GREEN, cfa_color::BLUE]
        || pattern.len() != 4
    {
        return Err(DngReaderError::UnsupportedCfa);
    }
    let cfa = match pattern.as_slice() {
        [
            cfa_color::RED,
            cfa_color::GREEN,
            cfa_color::GREEN,
            cfa_color::BLUE,
        ] => BayerCfa::Rggb,
        [
            cfa_color::GREEN,
            cfa_color::RED,
            cfa_color::BLUE,
            cfa_color::GREEN,
        ] => BayerCfa::Grbg,
        [
            cfa_color::GREEN,
            cfa_color::BLUE,
            cfa_color::RED,
            cfa_color::GREEN,
        ] => BayerCfa::Gbrg,
        [
            cfa_color::BLUE,
            cfa_color::GREEN,
            cfa_color::GREEN,
            cfa_color::RED,
        ] => BayerCfa::Bggr,
        _ => BayerCfa::Unsupported,
    };
    if cfa == BayerCfa::Unsupported {
        return Err(DngReaderError::UnsupportedCfa);
    }
    Ok(cfa)
}
/// Typed opcode payloads extracted from the raw IFD's opcode lists.
struct TypedOpcodes {
    warp_rectilinear: Vec<WarpRectilinearOpcode>,
    fix_vignette_radial: Vec<FixVignetteRadialOpcode>,
    gain_map: Vec<GainMapOpcode>,
}

fn metadata_from_decoded(
    decoded: &gamut_dng::DecodedDng,
    raw: &gamut_dng::RawImage,
    source_white_balance: SourceWhiteBalance,
    calibration: ColorCalibration,
    typed_opcodes: TypedOpcodes,
    opcode_lists: [DngOpcodeList; 3],
) -> DngMetadata {
    let levels = raw.levels();
    let exif = &decoded.metadata.exif;
    let (camera_calibration1, camera_calibration2) = decoded.profile.camera_calibration();
    let (forward_matrix1, forward_matrix2) = decoded.profile.forward_matrices();
    DngMetadata {
        dng_version: decoded.dng_version,
        backward_version: decoded.backward_version,
        black_repeat: levels.black_repeat(),
        black_levels: levels.black().to_vec(),
        black_delta_h: levels.black_delta_h().map(ToOwned::to_owned),
        black_delta_v: levels.black_delta_v().map(ToOwned::to_owned),
        white_levels: levels.white().to_vec(),
        linearization_table: levels.linearization_table().map(ToOwned::to_owned),
        camera_model: decoded.profile.unique_camera_model().to_owned(),
        color_matrix1: *decoded.profile.color_matrix1(),
        calibration_illuminant1: format!("{:?}", decoded.profile.calibration_illuminant1()),
        calibration_illuminant1_code: Some(decoded.profile.calibration_illuminant1().code()),
        calibration_illuminant2_code: decoded
            .profile
            .second_illuminant()
            .map(|(_, illuminant)| illuminant.code()),
        camera_calibration_signature: calibration.camera_calibration_signature,
        profile_calibration_signature: calibration.profile_calibration_signature,
        profile_hue_sat_map_dims: calibration.profile_hue_sat_map_dims,
        profile_hue_sat_map_data1: calibration.profile_hue_sat_map_data1,
        profile_hue_sat_map_data2: calibration.profile_hue_sat_map_data2,
        as_shot_neutral: source_white_balance.as_shot_neutral,
        as_shot_white_xy: source_white_balance.as_shot_white_xy,
        color_matrix2: source_white_balance.color_matrix2,
        camera_calibration1: camera_calibration1.copied(),
        camera_calibration2: camera_calibration2.copied(),
        forward_matrix1: forward_matrix1.copied(),
        forward_matrix2: forward_matrix2.copied(),
        analog_balance: decoded.profile.analog_balance().copied(),
        baseline_exposure: decoded.profile.baseline_exposure(),
        profile_name: decoded.profile.profile_name().map(ToOwned::to_owned),
        exif_exposure_time: exif.exposure_time,
        exif_f_number: exif.f_number,
        exif_iso_speed: exif.iso_speed,
        exif_brightness_value: exif_scalar(&decoded.exif_extra, BRIGHTNESS_VALUE_TAG),
        exif_exposure_bias_value: exif_scalar(&decoded.exif_extra, EXPOSURE_BIAS_VALUE_TAG),
        exif_date_time_original: exif.date_time_original.clone(),
        exif_focal_length: exif.focal_length,
        xmp_byte_length: decoded.metadata.xmp.as_ref().map(Vec::len),
        iptc_byte_length: decoded.metadata.iptc.as_ref().map(Vec::len),
        icc_byte_length: decoded.metadata.icc.as_ref().map(Vec::len),
        new_raw_image_digest: decoded.new_raw_image_digest.map(hex_bytes),
        ifd0_extra: decoded
            .ifd0_extra
            .iter()
            .filter(|tag| tag.tag != tags::AS_SHOT_WHITE_XY)
            .map(raw_tag)
            .collect(),
        raw_extra: decoded.raw_extra.iter().map(raw_tag).collect(),
        exif_extra: decoded.exif_extra.iter().map(raw_tag).collect(),
        metadata_hash: digest_bytes(&metadata_bytes(
            decoded,
            source_white_balance,
            &opcode_lists,
        )),
        warp_rectilinear: typed_opcodes.warp_rectilinear,
        fix_vignette_radial: typed_opcodes.fix_vignette_radial,
        gain_map: typed_opcodes.gain_map,
        opcode_lists,
    }
}

fn raw_opcode_lists(raw: &gamut_dng::RawImage) -> [DngOpcodeList; 3] {
    [raw.opcode_list1(), raw.opcode_list2(), raw.opcode_list3()].map(|list| DngOpcodeList {
        opcodes: list
            .opcodes()
            .iter()
            .map(|opcode| DngOpcode {
                id: opcode.id,
                spec_version: opcode.spec_version,
                flags: opcode.flags,
                parameters: opcode.parameters.clone(),
            })
            .collect(),
    })
}

fn parse_warp_rectilinear_opcodes(
    list: &OpcodeList,
) -> Result<Vec<WarpRectilinearOpcode>, DngReaderError> {
    list.opcodes()
        .iter()
        .enumerate()
        .filter(|(_, opcode)| opcode.id == opcode_id::WARP_RECTILINEAR)
        .map(|(opcode_index, opcode)| {
            let parameters = &opcode.parameters;
            let set_count =
                read_be_u32(parameters, 0).ok_or(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "missing coefficient-set count",
                })? as usize;
            if !matches!(set_count, 1 | 3) {
                return Err(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "coefficient-set count must be one or three for Bayer RGB",
                });
            }
            let expected_len = set_count
                .checked_mul(6)
                .and_then(|count| count.checked_mul(size_of::<f64>()))
                .and_then(|bytes| bytes.checked_add(4 + 2 * size_of::<f64>()))
                .ok_or(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "parameter byte length overflows",
                })?;
            if parameters.len() != expected_len {
                return Err(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "parameter byte length does not match coefficient-set count",
                });
            }

            let mut offset = 4;
            let mut coefficient_sets = Vec::with_capacity(set_count);
            for _ in 0..set_count {
                let mut values = [0.0; 6];
                for value in &mut values {
                    *value = read_be_f64(parameters, offset).ok_or(
                        DngReaderError::InvalidWarpRectilinear {
                            opcode_index,
                            reason: "truncated coefficient set",
                        },
                    )?;
                    offset += size_of::<f64>();
                }
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(DngReaderError::InvalidWarpRectilinear {
                        opcode_index,
                        reason: "coefficient values must be finite",
                    });
                }
                coefficient_sets.push(WarpRectilinearCoefficientSet {
                    radial: [values[0], values[1], values[2], values[3]],
                    tangential: [values[4], values[5]],
                });
            }

            let optical_center = [
                read_be_f64(parameters, offset).ok_or(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "missing optical-center x coordinate",
                })?,
                read_be_f64(parameters, offset + size_of::<f64>()).ok_or(
                    DngReaderError::InvalidWarpRectilinear {
                        opcode_index,
                        reason: "missing optical-center y coordinate",
                    },
                )?,
            ];
            if optical_center
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(DngReaderError::InvalidWarpRectilinear {
                    opcode_index,
                    reason: "optical-center coordinates must be finite and within [0, 1]",
                });
            }

            Ok(WarpRectilinearOpcode {
                spec_version: opcode.spec_version,
                flags: opcode.flags,
                coefficient_sets,
                optical_center,
            })
        })
        .collect()
}

fn parse_fix_vignette_radial_opcodes(
    list: &OpcodeList,
) -> Result<Vec<FixVignetteRadialOpcode>, DngReaderError> {
    const VALUE_COUNT: usize = 7;
    const PARAMETER_BYTES: usize = VALUE_COUNT * size_of::<f64>();

    list.opcodes()
        .iter()
        .enumerate()
        .filter(|(_, opcode)| opcode.id == opcode_id::FIX_VIGNETTE_RADIAL)
        .map(|(opcode_index, opcode)| {
            if opcode.parameters.len() != PARAMETER_BYTES {
                return Err(DngReaderError::InvalidFixVignetteRadial {
                    opcode_index,
                    reason: "parameter byte length must be exactly 56 bytes",
                });
            }
            let mut values = [0.0; VALUE_COUNT];
            for (index, value) in values.iter_mut().enumerate() {
                *value = read_be_f64(&opcode.parameters, index * size_of::<f64>()).ok_or(
                    DngReaderError::InvalidFixVignetteRadial {
                        opcode_index,
                        reason: "truncated parameter value",
                    },
                )?;
            }
            if values.iter().any(|value| !value.is_finite()) {
                return Err(DngReaderError::InvalidFixVignetteRadial {
                    opcode_index,
                    reason: "coefficient and optical-center values must be finite",
                });
            }
            let optical_center = [values[5], values[6]];
            if optical_center
                .iter()
                .any(|value| !(0.0..=1.0).contains(value))
            {
                return Err(DngReaderError::InvalidFixVignetteRadial {
                    opcode_index,
                    reason: "optical-center coordinates must be within [0, 1]",
                });
            }
            Ok(FixVignetteRadialOpcode {
                spec_version: opcode.spec_version,
                flags: opcode.flags,
                coefficients: values[..5]
                    .try_into()
                    .expect("five coefficient values are present"),
                optical_center,
            })
        })
        .collect()
}
/// Parses DNG `GainMap` opcodes (opcode id 9) from `OpcodeList2` into typed
/// metadata. Parameter layout per the Adobe DNG SDK (`dng_gain_map.cpp`):
/// `area_spec` (32 bytes) + mesh header (44 bytes) + `points_v * points_h *
/// map_planes` big-endian `f32` entries.
#[derive(Clone, Copy)]
struct GainMapGeometry {
    area: [i32; 4],
    row_pitch: u32,
    col_pitch: u32,
    points: [u32; 2],
    spacing: [f64; 2],
    origin: [f64; 2],
    map_planes: u32,
}

fn parse_gain_map_opcodes(list: &OpcodeList) -> Result<Vec<GainMapOpcode>, DngReaderError> {
    const AREA_SPEC_BYTES: usize = 32;
    const MESH_HEADER_BYTES: usize = 44;

    list.opcodes()
        .iter()
        .enumerate()
        .filter(|(_, opcode)| opcode.id == opcode_id::GAIN_MAP)
        .map(|(opcode_index, opcode)| {
            let parameters = &opcode.parameters;
            let header_bytes = AREA_SPEC_BYTES + MESH_HEADER_BYTES;
            if parameters.len() < header_bytes {
                return Err(DngReaderError::InvalidGainMap {
                    opcode_index,
                    reason: "parameter block shorter than the area spec and mesh header",
                });
            }
            let u32_at = |offset: usize| {
                u32::from_be_bytes(
                    parameters[offset..offset + 4]
                        .try_into()
                        .expect("offset is within the checked header"),
                )
            };
            let f64_at = |offset: usize| {
                f64::from_be_bytes(
                    parameters[offset..offset + 8]
                        .try_into()
                        .expect("offset is within the checked header"),
                )
            };
            let first_plane = u32_at(16);
            let plane_count = u32_at(20);
            let geometry = GainMapGeometry {
                area: [
                    i32::from_ne_bytes(u32_at(0).to_ne_bytes()),
                    i32::from_ne_bytes(u32_at(4).to_ne_bytes()),
                    i32::from_ne_bytes(u32_at(8).to_ne_bytes()),
                    i32::from_ne_bytes(u32_at(12).to_ne_bytes()),
                ],
                row_pitch: u32_at(24),
                col_pitch: u32_at(28),
                points: [u32_at(AREA_SPEC_BYTES), u32_at(AREA_SPEC_BYTES + 4)],
                spacing: [f64_at(AREA_SPEC_BYTES + 8), f64_at(AREA_SPEC_BYTES + 16)],
                origin: [f64_at(AREA_SPEC_BYTES + 24), f64_at(AREA_SPEC_BYTES + 32)],
                map_planes: u32_at(AREA_SPEC_BYTES + 40),
            };
            validate_gain_map_mesh(opcode_index, parameters, &geometry)?;
            let GainMapGeometry {
                area,
                row_pitch,
                col_pitch,
                points,
                spacing,
                origin,
                map_planes,
            } = geometry;
            let entries: Vec<f32> = parameters[header_bytes..]
                .chunks_exact(4)
                .map(|bytes| f32::from_be_bytes(bytes.try_into().expect("chunk is four bytes")))
                .collect();
            if entries.iter().any(|entry| !entry.is_finite()) {
                return Err(DngReaderError::InvalidGainMap {
                    opcode_index,
                    reason: "mesh entries must be finite",
                });
            }
            Ok(GainMapOpcode {
                spec_version: opcode.spec_version,
                flags: opcode.flags,
                area,
                first_plane,
                plane_count,
                row_pitch,
                col_pitch,
                points,
                spacing,
                origin,
                map_planes,
                entries,
            })
        })
        .collect()
}

/// Validates the mesh geometry and total parameter length of one `GainMap`
/// opcode; returns a stable `InvalidGainMap` error when the DNG is malformed.
fn validate_gain_map_mesh(
    opcode_index: usize,
    parameters: &[u8],
    geometry: &GainMapGeometry,
) -> Result<(), DngReaderError> {
    const AREA_SPEC_BYTES: usize = 32;
    const MESH_HEADER_BYTES: usize = 44;
    let header_bytes = AREA_SPEC_BYTES + MESH_HEADER_BYTES;
    let invalid = |reason: &'static str| DngReaderError::InvalidGainMap {
        opcode_index,
        reason,
    };
    let GainMapGeometry {
        area,
        row_pitch,
        col_pitch,
        points,
        spacing,
        origin,
        map_planes,
    } = *geometry;
    if points[0] == 0 || points[1] == 0 {
        return Err(invalid("mesh points must be at least 1 in each dimension"));
    }
    if map_planes == 0 {
        return Err(invalid("mesh plane count must be at least 1"));
    }
    if row_pitch == 0 || col_pitch == 0 {
        return Err(invalid("area pitch must be at least 1"));
    }
    let [top, left, bottom, right] = area;
    if bottom <= top || right <= left {
        if row_pitch != 1 || col_pitch != 1 {
            return Err(invalid("an empty area must use pitch 1"));
        }
    } else {
        let height = bottom
            .checked_sub(top)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("area height exceeds u32"))?;
        let width = right
            .checked_sub(left)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("area width exceeds u32"))?;
        if row_pitch > height || col_pitch > width {
            return Err(invalid("area pitch exceeds its bounds"));
        }
    }
    let entry_count = (usize::try_from(points[0])
        .expect("u32 fits usize on supported targets")
        .checked_mul(usize::try_from(points[1]).expect("u32 fits usize"))
        .and_then(|value| value.checked_mul(usize::try_from(map_planes).expect("u32 fits usize")))
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .and_then(|value| value.checked_add(header_bytes))
        .ok_or(DngReaderError::InvalidGainMap {
            opcode_index,
            reason: "declared mesh dimensions overflow",
        }))?;
    if parameters.len() != entry_count {
        return Err(invalid(
            "parameter byte length does not match the declared mesh dimensions",
        ));
    }
    if !(spacing[0].is_finite() && spacing[0] > 0.0 && spacing[1].is_finite() && spacing[1] > 0.0) {
        return Err(invalid("mesh spacing must be finite and positive"));
    }
    if !(origin[0].is_finite() && origin[1].is_finite()) {
        return Err(invalid("mesh origin must be finite"));
    }
    // Bit-exact comparisons against the exact spec values (spacing 1, origin 0)
    // are intentional: these are serialized f64 constants, not computed ones.
    if points[0] == 1
        && (spacing[0].to_bits() != f64::to_bits(1.0) || origin[0].to_bits() != f64::to_bits(0.0))
    {
        return Err(invalid("a single-row mesh must use spacing 1 and origin 0"));
    }
    if points[1] == 1
        && (spacing[1].to_bits() != f64::to_bits(1.0) || origin[1].to_bits() != f64::to_bits(0.0))
    {
        return Err(invalid(
            "a single-column mesh must use spacing 1 and origin 0",
        ));
    }
    Ok(())
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let value: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(value))
}

fn read_be_f64(bytes: &[u8], offset: usize) -> Option<f64> {
    let value: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(f64::from_be_bytes(value))
}

fn raw_tag(tag: &gamut_dng::RawTag) -> DngRawTag {
    DngRawTag {
        tag: tag.tag,
        field_type: tag.value.field_type().map_or_else(
            || "unknown".to_owned(),
            |field_type| format!("{field_type:?}"),
        ),
        count: tag.value.count(),
        value: format!("{:?}", tag.value),
    }
}

fn hex_bytes(bytes: [u8; 16]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(32), |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

#[derive(Clone, Copy, Debug)]
struct SourceWhiteBalance {
    as_shot_neutral: Option<[f64; 3]>,
    as_shot_white_xy: Option<[f64; 2]>,
    color_matrix2: Option<[f64; 9]>,
}

fn source_white_balance(data: &[u8]) -> gamut_dng::Result<SourceWhiteBalance> {
    let mut reader = IfdReader::open(data)?;
    let ifd0 = reader.read_ifd(reader.first_ifd_offset())?;
    Ok(SourceWhiteBalance {
        as_shot_neutral: read_rational_array(
            &mut reader,
            &ifd0,
            tags::AS_SHOT_NEUTRAL,
            "DNG: malformed AsShotNeutral",
        )?,
        as_shot_white_xy: read_rational_array(
            &mut reader,
            &ifd0,
            tags::AS_SHOT_WHITE_XY,
            "DNG: malformed AsShotWhiteXY",
        )?,
        color_matrix2: read_rational_array(
            &mut reader,
            &ifd0,
            tags::COLOR_MATRIX2,
            "DNG: malformed ColorMatrix2",
        )?,
    })
}

#[derive(Clone, Debug)]
struct ColorCalibration {
    camera_calibration_signature: Option<String>,
    profile_calibration_signature: Option<String>,
    profile_hue_sat_map_dims: Option<[u32; 3]>,
    profile_hue_sat_map_data1: Option<Vec<f32>>,
    profile_hue_sat_map_data2: Option<Vec<f32>>,
}

const CAMERA_CALIBRATION_SIGNATURE_TAG: u16 = 50931;
const PROFILE_CALIBRATION_SIGNATURE_TAG: u16 = 50932;

fn color_calibration(data: &[u8]) -> gamut_dng::Result<ColorCalibration> {
    let mut reader = IfdReader::open(data)?;
    let ifd0 = reader.read_ifd(reader.first_ifd_offset())?;
    let ascii = |reader: &mut IfdReader<&[u8]>, tag: u16| -> gamut_dng::Result<Option<String>> {
        match ifd0.entry(tag) {
            None => Ok(None),
            Some(entry) => Ok(reader.value(entry)?.as_str().map(ToOwned::to_owned)),
        }
    };
    let dims = match ifd0.entry(tags::PROFILE_HUE_SAT_MAP_DIMS) {
        None => None,
        Some(entry) => {
            let value = reader.value(entry)?;
            let codes = value.as_u32_vec().ok_or(gamut_dng::Error::InvalidInput(
                "DNG: malformed ProfileHueSatMapDims",
            ))?;
            if codes.len() != 3 {
                return Err(gamut_dng::Error::InvalidInput(
                    "DNG: ProfileHueSatMapDims must have three entries",
                ));
            }
            Some([codes[0], codes[1], codes[2]])
        }
    };
    let floats = |reader: &mut IfdReader<&[u8]>, tag: u16| -> gamut_dng::Result<Option<Vec<f32>>> {
        match ifd0.entry(tag) {
            None => Ok(None),
            Some(entry) => match reader.value(entry)? {
                gamut_ifd::Value::Float(values) => Ok(Some(values)),
                _ => Err(gamut_dng::Error::InvalidInput(
                    "DNG: ProfileHueSatMapData must be FLOAT",
                )),
            },
        }
    };
    Ok(ColorCalibration {
        camera_calibration_signature: ascii(&mut reader, CAMERA_CALIBRATION_SIGNATURE_TAG)?,
        profile_calibration_signature: ascii(&mut reader, PROFILE_CALIBRATION_SIGNATURE_TAG)?,
        profile_hue_sat_map_dims: dims,
        profile_hue_sat_map_data1: floats(&mut reader, tags::PROFILE_HUE_SAT_MAP_DATA1)?,
        profile_hue_sat_map_data2: floats(&mut reader, tags::PROFILE_HUE_SAT_MAP_DATA2)?,
    })
}

fn read_rational_array<const N: usize, S: ReadAt>(
    reader: &mut IfdReader<S>,
    ifd: &RawIfd,
    tag: u16,
    malformed: &'static str,
) -> gamut_dng::Result<Option<[f64; N]>> {
    let Some(entry) = ifd.entry(tag) else {
        return Ok(None);
    };
    let value = reader.value(entry)?;
    rational_array(Some(&value))
        .map(Some)
        .ok_or(gamut_dng::Error::InvalidInput(malformed))
}

fn decoder_compatible_data(
    data: &[u8],
    source: SourceWhiteBalance,
) -> gamut_dng::Result<Cow<'_, [u8]>> {
    if source.as_shot_neutral.is_some() || source.as_shot_white_xy.is_none() {
        return Ok(Cow::Borrowed(data));
    }

    let mut reader = IfdReader::open(data)?;
    let order = reader.order();
    let variant = reader.variant();
    let ifd0 = reader.read_ifd(reader.first_ifd_offset())?;
    let entry = ifd0
        .entry(tags::AS_SHOT_WHITE_XY)
        .ok_or(gamut_dng::Error::InvalidInput("DNG: missing AsShotWhiteXY"))?;
    let entry_offset = usize::try_from(entry.offset)
        .map_err(|_| gamut_dng::Error::InvalidInput("DNG: IFD entry offset overflow"))?;

    let mut compatible = data.to_vec();
    if compatible.len() & 1 != 0 {
        compatible.push(0);
    }
    let neutral_offset = compatible.len() as u64;
    for _ in 0..3 {
        compatible.extend_from_slice(&order.pack_u32(1));
        compatible.extend_from_slice(&order.pack_u32(1));
    }

    write_at(
        &mut compatible,
        entry_offset,
        &order.pack_u16(tags::AS_SHOT_NEUTRAL),
    )?;
    match variant {
        Variant::Classic => {
            write_at(&mut compatible, entry_offset + 4, &order.pack_u32(3))?;
            let offset = u32::try_from(neutral_offset).map_err(|_| {
                gamut_dng::Error::InvalidInput("DNG: synthetic neutral offset exceeds classic TIFF")
            })?;
            write_at(&mut compatible, entry_offset + 8, &order.pack_u32(offset))?;
        }
        Variant::Big => {
            write_at(&mut compatible, entry_offset + 4, &order.pack_u64(3))?;
            write_at(
                &mut compatible,
                entry_offset + 12,
                &order.pack_u64(neutral_offset),
            )?;
        }
    }
    Ok(Cow::Owned(compatible))
}

fn write_at(data: &mut [u8], offset: usize, value: &[u8]) -> gamut_dng::Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(gamut_dng::Error::InvalidInput(
            "DNG: IFD entry offset overflow",
        ))?;
    let target = data
        .get_mut(offset..end)
        .ok_or(gamut_dng::Error::InvalidInput(
            "DNG: IFD entry out of bounds",
        ))?;
    target.copy_from_slice(value);
    Ok(())
}

fn rational_array<const N: usize>(value: Option<&Value>) -> Option<[f64; N]> {
    let values: Vec<f64> = if let Some(rationals) = value?.as_rationals() {
        rationals
            .iter()
            .map(|&(numerator, denominator)| ratio(f64::from(numerator), f64::from(denominator)))
            .collect()
    } else {
        value?
            .as_srationals()?
            .iter()
            .map(|&(numerator, denominator)| ratio(f64::from(numerator), f64::from(denominator)))
            .collect()
    };
    values.try_into().ok()
}

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn metadata_bytes(
    decoded: &gamut_dng::DecodedDng,
    source_white_balance: SourceWhiteBalance,
    opcode_lists: &[DngOpcodeList; 3],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&decoded.dng_version);
    bytes.extend_from_slice(decoded.profile.unique_camera_model().as_bytes());
    for value in decoded.profile.color_matrix1() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    if let Some(neutral) = source_white_balance.as_shot_neutral {
        for value in neutral {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    if let Some(white_xy) = source_white_balance.as_shot_white_xy {
        for value in white_xy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for (list_index, list) in opcode_lists.iter().enumerate() {
        bytes.push(u8::try_from(list_index).expect("three opcode lists fit u8"));
        bytes.extend_from_slice(
            &u32::try_from(list.opcodes.len())
                .expect("opcode-list length was decoded from u32")
                .to_le_bytes(),
        );
        for opcode in &list.opcodes {
            bytes.extend_from_slice(&opcode.id.to_le_bytes());
            bytes.extend_from_slice(&opcode.spec_version);
            bytes.extend_from_slice(&opcode.flags.to_le_bytes());
            bytes.extend_from_slice(
                &u32::try_from(opcode.parameters.len())
                    .expect("opcode parameter length was decoded from u32")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&opcode.parameters);
        }
    }
    bytes
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest_u16(samples: &[u16]) -> String {
    #[cfg(target_endian = "little")]
    {
        digest_bytes(bytemuck::cast_slice(samples))
    }
    #[cfg(target_endian = "big")]
    {
        let mut hasher = Sha256::new();
        for sample in samples {
            hasher.update(sample.to_le_bytes());
        }
        format!("{hasher:x}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BRIGHTNESS_VALUE_TAG, DngReaderError, EXPOSURE_BIAS_VALUE_TAG, exif_scalar,
        parse_fix_vignette_radial_opcodes, parse_warp_rectilinear_opcodes,
    };
    use gamut_dng::{Opcode, OpcodeList, RawTag, Value, opcode_id};

    fn warp_rectilinear_parameters(
        coefficient_sets: &[[f64; 6]],
        optical_center: [f64; 2],
    ) -> Vec<u8> {
        let mut parameters = Vec::new();
        parameters.extend_from_slice(
            &u32::try_from(coefficient_sets.len())
                .expect("test coefficient count fits u32")
                .to_be_bytes(),
        );
        for coefficients in coefficient_sets {
            for coefficient in coefficients {
                parameters.extend_from_slice(&coefficient.to_be_bytes());
            }
        }
        for coordinate in optical_center {
            parameters.extend_from_slice(&coordinate.to_be_bytes());
        }
        parameters
    }

    fn opcode_list(parameters: Vec<u8>) -> OpcodeList {
        let mut list = OpcodeList::new();
        list.push(Opcode {
            id: opcode_id::WARP_RECTILINEAR,
            spec_version: [1, 3, 0, 0],
            flags: Opcode::FLAG_OPTIONAL,
            parameters,
        });
        list
    }

    fn fix_vignette_parameters(coefficients: [f64; 5], optical_center: [f64; 2]) -> Vec<u8> {
        coefficients
            .into_iter()
            .chain(optical_center)
            .flat_map(f64::to_be_bytes)
            .collect()
    }

    fn fix_vignette_opcode_list(parameters: Vec<u8>) -> OpcodeList {
        let mut list = OpcodeList::new();
        list.push(Opcode {
            id: opcode_id::FIX_VIGNETTE_RADIAL,
            spec_version: [1, 3, 0, 0],
            flags: Opcode::FLAG_OPTIONAL,
            parameters,
        });
        list
    }

    #[test]
    fn reads_signed_and_unsigned_exif_apex_values() {
        let tags = vec![
            RawTag {
                tag: BRIGHTNESS_VALUE_TAG,
                value: Value::SRational(vec![(9, 2)]),
            },
            RawTag {
                tag: EXPOSURE_BIAS_VALUE_TAG,
                value: Value::SRational(vec![(-3, 2)]),
            },
        ];

        assert_eq!(exif_scalar(&tags, BRIGHTNESS_VALUE_TAG), Some(4.5));
        assert_eq!(exif_scalar(&tags, EXPOSURE_BIAS_VALUE_TAG), Some(-1.5));
    }

    #[test]
    fn rejects_missing_or_zero_denominator_exif_values() {
        let tags = vec![RawTag {
            tag: BRIGHTNESS_VALUE_TAG,
            value: Value::Rational(vec![(1, 0)]),
        }];

        assert_eq!(exif_scalar(&tags, EXPOSURE_BIAS_VALUE_TAG), None);
        assert_eq!(exif_scalar(&tags, BRIGHTNESS_VALUE_TAG), None);
    }

    #[test]
    fn decodes_warp_rectilinear_parameters_as_big_endian_values() {
        let coefficients = [
            [1.0, 0.1, 0.01, 0.001, 0.0001, 0.00001],
            [1.0, 0.2, 0.02, 0.002, 0.0002, 0.00002],
            [1.0, 0.3, 0.03, 0.003, 0.0003, 0.00003],
        ];
        let list = opcode_list(warp_rectilinear_parameters(&coefficients, [0.49, 0.51]));

        let decoded = parse_warp_rectilinear_opcodes(&list).expect("valid opcode");

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].spec_version, [1, 3, 0, 0]);
        assert_eq!(decoded[0].flags, Opcode::FLAG_OPTIONAL);
        assert_eq!(decoded[0].coefficient_sets.len(), 3);
        assert_eq!(
            decoded[0].coefficient_sets[1].radial.map(f64::to_bits),
            [1.0, 0.2, 0.02, 0.002].map(f64::to_bits)
        );
        assert_eq!(
            decoded[0].coefficient_sets[1].tangential.map(f64::to_bits),
            [0.0002, 0.00002].map(f64::to_bits)
        );
        assert_eq!(
            decoded[0].optical_center.map(f64::to_bits),
            [0.49, 0.51].map(f64::to_bits)
        );
    }

    #[test]
    fn rejects_warp_rectilinear_parameter_size_mismatch() {
        let mut parameters =
            warp_rectilinear_parameters(&[[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]], [0.5, 0.5]);
        parameters.pop();
        let error = parse_warp_rectilinear_opcodes(&opcode_list(parameters))
            .expect_err("truncated optical center must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidWarpRectilinear { .. }
        ));
    }

    #[test]
    fn rejects_empty_warp_rectilinear_coefficient_sets() {
        let parameters = warp_rectilinear_parameters(&[], [0.5, 0.5]);
        let error = parse_warp_rectilinear_opcodes(&opcode_list(parameters))
            .expect_err("zero coefficient sets must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidWarpRectilinear { .. }
        ));
    }

    #[test]
    fn rejects_non_rgb_warp_rectilinear_coefficient_counts() {
        let coefficients = [
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0, 0.1, 0.0, 0.0, 0.0, 0.0],
        ];
        let parameters = warp_rectilinear_parameters(&coefficients, [0.5, 0.5]);

        let error = parse_warp_rectilinear_opcodes(&opcode_list(parameters))
            .expect_err("Bayer RGB requires one shared or three plane coefficient sets");

        assert!(matches!(
            error,
            DngReaderError::InvalidWarpRectilinear { .. }
        ));
    }

    #[test]
    fn rejects_warp_rectilinear_center_outside_image() {
        let parameters =
            warp_rectilinear_parameters(&[[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]], [1.01, 0.5]);
        let error = parse_warp_rectilinear_opcodes(&opcode_list(parameters))
            .expect_err("out-of-range optical center must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidWarpRectilinear { .. }
        ));
    }

    #[test]
    fn rejects_warp_rectilinear_non_finite_values() {
        let parameters =
            warp_rectilinear_parameters(&[[1.0, 0.0, f64::NAN, 0.0, 0.0, 0.0]], [0.5, 0.5]);
        let error = parse_warp_rectilinear_opcodes(&opcode_list(parameters))
            .expect_err("non-finite coefficient must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidWarpRectilinear { .. }
        ));
    }

    #[test]
    fn ignores_other_opcode_ids_without_reordering_warps() {
        let first = warp_rectilinear_parameters(&[[1.0, 0.1, 0.0, 0.0, 0.0, 0.0]], [0.5, 0.5]);
        let second = warp_rectilinear_parameters(&[[1.0, 0.2, 0.0, 0.0, 0.0, 0.0]], [0.5, 0.5]);
        let mut list = opcode_list(first);
        list.push(Opcode {
            id: opcode_id::TRIM_BOUNDS,
            spec_version: [1, 3, 0, 0],
            flags: 0,
            parameters: vec![0; 16],
        });
        list.push(Opcode {
            id: opcode_id::WARP_RECTILINEAR,
            spec_version: [1, 3, 0, 0],
            flags: 0,
            parameters: second,
        });

        let decoded = parse_warp_rectilinear_opcodes(&list).expect("valid opcodes");

        assert_eq!(decoded.len(), 2);
        assert_eq!(
            decoded[0].coefficient_sets[0].radial[1].to_bits(),
            0.1_f64.to_bits()
        );
        assert_eq!(
            decoded[1].coefficient_sets[0].radial[1].to_bits(),
            0.2_f64.to_bits()
        );
    }

    #[test]
    fn decodes_fix_vignette_radial_parameters_as_big_endian_values() {
        let list = fix_vignette_opcode_list(fix_vignette_parameters(
            [0.1, 0.02, 0.003, 0.0004, 0.00005],
            [0.49, 0.51],
        ));

        let decoded = parse_fix_vignette_radial_opcodes(&list).expect("valid opcode");

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].spec_version, [1, 3, 0, 0]);
        assert_eq!(decoded[0].flags, Opcode::FLAG_OPTIONAL);
        assert_eq!(
            decoded[0].coefficients.map(f64::to_bits),
            [0.1, 0.02, 0.003, 0.0004, 0.00005].map(f64::to_bits)
        );
        assert_eq!(
            decoded[0].optical_center.map(f64::to_bits),
            [0.49, 0.51].map(f64::to_bits)
        );
    }

    #[test]
    fn rejects_fix_vignette_radial_parameter_size_mismatch() {
        let mut parameters = fix_vignette_parameters([0.0; 5], [0.5, 0.5]);
        parameters.pop();

        let error = parse_fix_vignette_radial_opcodes(&fix_vignette_opcode_list(parameters))
            .expect_err("truncated optical center must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidFixVignetteRadial { .. }
        ));
    }

    #[test]
    fn rejects_fix_vignette_radial_non_finite_values() {
        let parameters = fix_vignette_parameters([0.0, f64::NAN, 0.0, 0.0, 0.0], [0.5, 0.5]);

        let error = parse_fix_vignette_radial_opcodes(&fix_vignette_opcode_list(parameters))
            .expect_err("non-finite coefficient must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidFixVignetteRadial { .. }
        ));
    }

    #[test]
    fn rejects_fix_vignette_radial_center_outside_image() {
        let parameters = fix_vignette_parameters([0.0; 5], [-0.01, 0.5]);

        let error = parse_fix_vignette_radial_opcodes(&fix_vignette_opcode_list(parameters))
            .expect_err("out-of-range optical center must fail");

        assert!(matches!(
            error,
            DngReaderError::InvalidFixVignetteRadial { .. }
        ));
    }

    #[test]
    fn preserves_fix_vignette_radial_opcode_order() {
        let first = fix_vignette_parameters([0.1, 0.0, 0.0, 0.0, 0.0], [0.5, 0.5]);
        let second = fix_vignette_parameters([0.2, 0.0, 0.0, 0.0, 0.0], [0.4, 0.6]);
        let mut list = fix_vignette_opcode_list(first);
        list.push(Opcode {
            id: opcode_id::TRIM_BOUNDS,
            spec_version: [1, 3, 0, 0],
            flags: 0,
            parameters: vec![0; 16],
        });
        list.push(Opcode {
            id: opcode_id::FIX_VIGNETTE_RADIAL,
            spec_version: [1, 3, 0, 0],
            flags: 0,
            parameters: second,
        });

        let decoded = parse_fix_vignette_radial_opcodes(&list).expect("valid opcodes");

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].coefficients[0].to_bits(), 0.1_f64.to_bits());
        assert_eq!(decoded[1].coefficients[0].to_bits(), 0.2_f64.to_bits());
        assert_eq!(
            decoded[1].optical_center.map(f64::to_bits),
            [0.4, 0.6].map(f64::to_bits)
        );
    }
}
