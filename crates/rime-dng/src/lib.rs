#![forbid(unsafe_code)]

use std::borrow::Cow;

use std::path::{Path, PathBuf};

use gamut_dng::{DngDecoder, RawPhotometry, Value, cfa_color, tags};
use gamut_ifd::{IfdReader, RawIfd, ReadAt, Variant};
use sha2::{Digest, Sha256};
use thiserror::Error;

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
    pub exif_date_time_original: Option<String>,
    pub exif_focal_length: Option<(u32, u32)>,
    pub xmp_byte_length: Option<usize>,
    pub iptc_byte_length: Option<usize>,
    pub icc_byte_length: Option<usize>,
    pub new_raw_image_digest: Option<String>,
    pub ifd0_extra: Vec<DngRawTag>,
    pub raw_extra: Vec<DngRawTag>,
    pub exif_extra: Vec<DngRawTag>,
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
        let metadata = metadata_from_decoded(&decoded, raw, source_white_balance);
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
    if *repeat != (2, 2) || plane_color.len() != 3 || pattern.len() != 4 {
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

fn metadata_from_decoded(
    decoded: &gamut_dng::DecodedDng,
    raw: &gamut_dng::RawImage,
    source_white_balance: SourceWhiteBalance,
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
        metadata_hash: digest_bytes(&metadata_bytes(decoded, source_white_balance)),
    }
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
