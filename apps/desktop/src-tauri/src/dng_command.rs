use std::cmp::Ordering;
use std::path::Path;

use rime_dng::DngReader;
use rime_isp::vfe::white_balance::{WhiteBalanceMetadata, white_balance_gains};
use serde::Serialize;
use tauri::ipc::Response;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DngRawTagDescriptor {
    pub tag: u16,
    pub field_type: String,
    pub count: u64,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DngMetadataDescriptor {
    pub dng_version: [u8; 4],
    pub backward_version: Option<[u8; 4]>,
    pub black_repeat: [u16; 2],
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
    pub exif_exposure_time: Option<[u32; 2]>,
    pub exif_f_number: Option<[u32; 2]>,
    pub exif_iso_speed: Option<u16>,
    pub exif_brightness_value: Option<f64>,
    pub exif_exposure_bias_value: Option<f64>,
    pub exif_date_time_original: Option<String>,
    pub exif_focal_length: Option<[u32; 2]>,
    pub xmp_byte_length: Option<usize>,
    pub iptc_byte_length: Option<usize>,
    pub icc_byte_length: Option<usize>,
    pub new_raw_image_digest: Option<String>,
    pub ifd0_extra: Vec<DngRawTagDescriptor>,
    pub raw_extra: Vec<DngRawTagDescriptor>,
    pub exif_extra: Vec<DngRawTagDescriptor>,
}

/// Invocation-frozen color reproduce assets solved from DNG metadata by the
/// `rime-isp` preprocess (single source of truth shared with the native GPU
/// path) and forwarded to the WebGPU fused preview.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorReproduceAssets {
    pub sensor_to_prophoto: [f32; 9],
    pub prophoto_to_srgb: [f32; 9],
    pub hs_dims: [u32; 2],
    pub hs_enable: bool,
    pub hs_lut: Option<Vec<f32>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DngFrameDescriptor {
    pub frame_index: u64,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub row_stride_samples: u32,
    pub storage_bits: u8,
    pub cfa: String,
    pub black_level: f32,
    pub white_level: f32,
    pub dng_version: [u8; 4],
    pub backward_version: Option<[u8; 4]>,
    pub camera_model: String,
    pub metadata_hash: String,
    pub raw_digest: String,
    pub white_balance_gains: [f32; 3],
    pub color_reproduce: ColorReproduceAssets,
    pub metadata: DngMetadataDescriptor,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DngSequenceDescriptor {
    pub directory: String,
    pub paths: Vec<String>,
    pub file_names: Vec<String>,
    pub frame_count: usize,
}

#[tauri::command]
pub fn list_dng_sequence(path: String) -> Result<DngSequenceDescriptor, String> {
    let selected = Path::new(&path);
    let directory = selected
        .parent()
        .ok_or_else(|| "DNG_SEQUENCE_INVALID: selected DNG has no parent directory".to_owned())?;
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| format!("DNG_SEQUENCE_READ_FAILED: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dng"))
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| natural_path_cmp(left, right));
    if entries.is_empty() {
        return Err(format!(
            "DNG_SEQUENCE_EMPTY: no DNG files found in {}",
            directory.display()
        ));
    }
    let file_names = entries
        .iter()
        .map(|entry| {
            entry.file_name().map_or_else(
                || entry.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        })
        .collect::<Vec<_>>();
    let paths = entries
        .iter()
        .map(|entry| entry.display().to_string())
        .collect::<Vec<_>>();
    Ok(DngSequenceDescriptor {
        directory: directory.display().to_string(),
        frame_count: paths.len(),
        paths,
        file_names,
    })
}

fn natural_path_cmp(left: &Path, right: &Path) -> Ordering {
    let left_name = left.file_name().map_or_else(
        || left.as_os_str().to_string_lossy(),
        |name| name.to_string_lossy(),
    );
    let right_name = right.file_name().map_or_else(
        || right.as_os_str().to_string_lossy(),
        |name| name.to_string_lossy(),
    );
    natural_str_cmp(&left_name, &right_name)
}

fn natural_str_cmp(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = digit_run_end(left, left_index);
            let right_end = digit_run_end(right, right_index);
            let left_significant = significant_digits(left, left_index, left_end);
            let right_significant = significant_digits(right, right_index, right_end);
            let numeric_order = left_significant.len().cmp(&right_significant.len());
            if numeric_order != Ordering::Equal {
                return numeric_order;
            }
            let numeric_order = left_significant.cmp(right_significant);
            if numeric_order != Ordering::Equal {
                return numeric_order;
            }
            let width_order = (left_end - left_index).cmp(&(right_end - right_index));
            if width_order != Ordering::Equal {
                return width_order;
            }
            left_index = left_end;
            right_index = right_end;
        } else {
            let order = left[left_index]
                .to_ascii_lowercase()
                .cmp(&right[right_index].to_ascii_lowercase());
            if order != Ordering::Equal {
                return order;
            }
            left_index += 1;
            right_index += 1;
        }
    }
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn digit_run_end(value: &[u8], mut index: usize) -> usize {
    while index < value.len() && value[index].is_ascii_digit() {
        index += 1;
    }
    index
}

fn significant_digits(value: &[u8], mut start: usize, end: usize) -> &[u8] {
    while start < end && value[start] == b'0' {
        start += 1;
    }
    &value[start..end]
}

#[tauri::command]
pub fn read_dng_frame(path: String, frame_index: Option<u64>) -> Result<Response, String> {
    let path = Path::new(&path);
    let frame = DngReader::new()
        .decode_file(path, frame_index.unwrap_or(0))
        .map_err(|error| error.to_string())?;
    Ok(Response::new(dng_frame_payload(&frame, path)?))
}

fn dng_frame_payload(frame: &rime_dng::DecodedRawFrame, path: &Path) -> Result<Vec<u8>, String> {
    let descriptor = serde_json::to_vec(&descriptor_from_frame(frame, path)?)
        .map_err(|error| format!("DNG_DESCRIPTOR_ENCODE_FAILED: {error}"))?;
    let descriptor_length = u32::try_from(descriptor.len())
        .map_err(|_| "DNG_DESCRIPTOR_ENCODE_FAILED: descriptor exceeds u32 length".to_owned())?;
    let padding = descriptor.len() & 1;
    let sample_bytes = frame.sample_bytes_le();
    let mut payload = Vec::with_capacity(4 + descriptor.len() + padding + sample_bytes.len());
    payload.extend_from_slice(&descriptor_length.to_le_bytes());
    payload.extend_from_slice(&descriptor);
    if padding != 0 {
        payload.push(0);
    }
    payload.extend_from_slice(&sample_bytes);
    Ok(payload)
}

fn color_reproduce_assets(
    frame: &rime_dng::DecodedRawFrame,
) -> Result<ColorReproduceAssets, String> {
    let metadata = &frame.metadata;
    let context = rime_isp::PreprocessContext {
        identity: rime_isp::FrameIdentity {
            frame_index: frame.frame_index,
            run_revision: 1,
            method_revision: 1,
        },
        width: frame.layout.width,
        height: frame.layout.height,
        black_level: metadata.black_levels.first().copied().unwrap_or(0.0) as f32,
        white_level: metadata.white_levels.first().copied().unwrap_or(1.0) as f32,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: metadata.as_shot_neutral,
        as_shot_white_xy: metadata.as_shot_white_xy,
        color_matrix1: metadata.color_matrix1,
        color_matrix2: metadata.color_matrix2,
        calibration_illuminant1_code: metadata.calibration_illuminant1_code,
        calibration_illuminant2_code: metadata.calibration_illuminant2_code,
        camera_calibration1: metadata.camera_calibration1,
        camera_calibration2: metadata.camera_calibration2,
        camera_calibration_signature: metadata.camera_calibration_signature.clone(),
        profile_calibration_signature: metadata.profile_calibration_signature.clone(),
        profile_hue_sat_map_dims: metadata.profile_hue_sat_map_dims,
        profile_hue_sat_map_data1: metadata.profile_hue_sat_map_data1.clone(),
        profile_hue_sat_map_data2: metadata.profile_hue_sat_map_data2.clone(),
        analog_balance: metadata.analog_balance,
        scene_brightness_ev: metadata.exif_brightness_value,
        exposure_deviation_ev: metadata.exif_exposure_bias_value,
        iso: metadata.exif_iso_speed.map(f64::from),
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: metadata.baseline_exposure,
        exposure_time_seconds: None,
        f_number: None,
        drc_local_statistics: None,
        drc_exposure_policy: rime_isp::vbe::drc::DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
        drc_gain_offset_ev: None,
        drc_knee: None,
        drc_amplifier: None,
        wbc_highlight_recovery: false,
    };
    let operator = rime_isp::operator_by_id("color_reproduce")
        .ok_or_else(|| "DNG_COLOR_REPRODUCE_INVALID: operator missing".to_owned())?;
    let packet = operator
        .preprocess("00", &context)
        .map_err(|error| format!("DNG_COLOR_REPRODUCE_INVALID: {error}"))?;

    let matrices = packet
        .resource("cr_matrices")
        .ok_or_else(|| "DNG_COLOR_REPRODUCE_INVALID: matrices missing".to_owned())?;
    let bytes = matrices.bytes();
    let read = |index: usize| {
        f32::from_ne_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("f32 slice"),
        )
    };
    let mut sensor_to_prophoto = [0.0_f32; 9];
    let mut prophoto_to_srgb = [0.0_f32; 9];
    for index in 0..9 {
        sensor_to_prophoto[index] = read(index);
        prophoto_to_srgb[index] = read(9 + index);
    }
    let uniform = packet.bytes();
    let dim = |slot: usize| {
        u32::from_ne_bytes(
            uniform[slot * 4..slot * 4 + 4]
                .try_into()
                .expect("u32 slice"),
        )
    };
    let hs_enable = dim(2) != 0;
    let hs_lut = packet.resource("cr_hs_lut").map(|lut| {
        lut.bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| f32::from_ne_bytes(*chunk))
            .collect::<Vec<f32>>()
    });
    Ok(ColorReproduceAssets {
        sensor_to_prophoto,
        prophoto_to_srgb,
        hs_dims: [dim(0), dim(1)],
        hs_enable,
        hs_lut,
    })
}

pub fn descriptor_from_frame(
    frame: &rime_dng::DecodedRawFrame,
    path: &Path,
) -> Result<DngFrameDescriptor, String> {
    let metadata = &frame.metadata;
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: metadata.as_shot_neutral,
        as_shot_white_xy: metadata.as_shot_white_xy,
        color_matrix1: metadata.color_matrix1,
        color_matrix2: metadata.color_matrix2,
        analog_balance: metadata.analog_balance,
    })
    .map_err(|error| format!("DNG_WHITE_BALANCE_INVALID: {error}"))?;
    Ok(DngFrameDescriptor {
        frame_index: frame.frame_index,
        file_name: path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        width: frame.layout.width,
        height: frame.layout.height,
        row_stride_samples: frame.layout.row_stride_samples,
        storage_bits: frame.layout.storage_bits,
        cfa: format!("{:?}", frame.layout.cfa).to_lowercase(),
        black_level: metadata.black_levels.first().copied().unwrap_or(0.0) as f32,
        white_level: metadata.white_levels.first().copied().unwrap_or(0.0) as f32,
        dng_version: metadata.dng_version,
        backward_version: metadata.backward_version,
        camera_model: metadata.camera_model.clone(),
        metadata_hash: metadata.metadata_hash.clone(),
        raw_digest: frame.raw_digest.clone(),
        white_balance_gains: [gains.red, gains.green, gains.blue],
        color_reproduce: color_reproduce_assets(frame)?,
        metadata: metadata_descriptor(metadata),
    })
}

fn metadata_descriptor(metadata: &rime_dng::DngMetadata) -> DngMetadataDescriptor {
    DngMetadataDescriptor {
        dng_version: metadata.dng_version,
        backward_version: metadata.backward_version,
        black_repeat: [metadata.black_repeat.0, metadata.black_repeat.1],
        black_levels: metadata.black_levels.clone(),
        black_delta_h: metadata.black_delta_h.clone(),
        black_delta_v: metadata.black_delta_v.clone(),
        white_levels: metadata.white_levels.clone(),
        linearization_table: metadata.linearization_table.clone(),
        camera_model: metadata.camera_model.clone(),
        color_matrix1: metadata.color_matrix1,
        calibration_illuminant1: metadata.calibration_illuminant1.clone(),
        as_shot_neutral: metadata.as_shot_neutral,
        as_shot_white_xy: metadata.as_shot_white_xy,
        color_matrix2: metadata.color_matrix2,
        camera_calibration1: metadata.camera_calibration1,
        camera_calibration2: metadata.camera_calibration2,
        forward_matrix1: metadata.forward_matrix1,
        forward_matrix2: metadata.forward_matrix2,
        analog_balance: metadata.analog_balance,
        baseline_exposure: metadata.baseline_exposure,
        profile_name: metadata.profile_name.clone(),
        exif_exposure_time: metadata.exif_exposure_time.map(tuple_to_array),
        exif_f_number: metadata.exif_f_number.map(tuple_to_array),
        exif_iso_speed: metadata.exif_iso_speed,
        exif_brightness_value: metadata.exif_brightness_value,
        exif_exposure_bias_value: metadata.exif_exposure_bias_value,
        exif_date_time_original: metadata.exif_date_time_original.clone(),
        exif_focal_length: metadata.exif_focal_length.map(tuple_to_array),
        xmp_byte_length: metadata.xmp_byte_length,
        iptc_byte_length: metadata.iptc_byte_length,
        icc_byte_length: metadata.icc_byte_length,
        new_raw_image_digest: metadata.new_raw_image_digest.clone(),
        ifd0_extra: metadata.ifd0_extra.iter().map(raw_tag_descriptor).collect(),
        raw_extra: metadata.raw_extra.iter().map(raw_tag_descriptor).collect(),
        exif_extra: metadata.exif_extra.iter().map(raw_tag_descriptor).collect(),
    }
}

fn tuple_to_array(value: (u32, u32)) -> [u32; 2] {
    [value.0, value.1]
}
fn raw_tag_descriptor(tag: &rime_dng::DngRawTag) -> DngRawTagDescriptor {
    DngRawTagDescriptor {
        tag: tag.tag,
        field_type: tag.field_type.clone(),
        count: tag.count,
        value: tag.value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GH5S: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../pipeline/normal/P1020601.dng"
    );
    #[test]
    fn gh5s_descriptor_and_binary_length_match() {
        let frame = DngReader::new()
            .decode_file(Path::new(GH5S), 0)
            .expect("GH5S frame must decode");
        let descriptor = descriptor_from_frame(&frame, Path::new(GH5S))
            .expect("descriptor must preprocess white balance");
        let payload =
            dng_frame_payload(&frame, Path::new(GH5S)).expect("frame payload must encode");
        let descriptor_length =
            u32::from_le_bytes(payload[0..4].try_into().expect("length header")) as usize;
        let raw_offset = 4 + descriptor_length + (descriptor_length & 1);
        assert_eq!(descriptor.width, 3744);
        assert_eq!(descriptor.height, 2776);
        assert_eq!(payload.len() - raw_offset, frame.samples().len() * 2);
        assert!(!descriptor.metadata.raw_extra.is_empty());
    }

    #[test]
    fn dng_frame_payload_contains_descriptor_and_raw_samples() {
        let frame = DngReader::new()
            .decode_file(Path::new(GH5S), 9)
            .expect("GH5S frame must decode");

        let payload =
            dng_frame_payload(&frame, Path::new(GH5S)).expect("frame payload must encode");
        let descriptor_length =
            u32::from_le_bytes(payload[0..4].try_into().expect("length header")) as usize;
        let raw_offset = 4 + descriptor_length + (descriptor_length & 1);
        let descriptor: serde_json::Value =
            serde_json::from_slice(&payload[4..4 + descriptor_length]).expect("descriptor JSON");

        assert_eq!(descriptor["frameIndex"], 9);
        assert_eq!(payload.len() - raw_offset, frame.samples().len() * 2);
    }

    #[test]
    fn dng_sequence_filters_extensions_and_naturally_sorts_names() {
        let directory =
            std::env::temp_dir().join(format!("rime-dng-sequence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).expect("temporary directory must be created");
        for name in ["frame10.dng", "frame2.DNG", "frame1.dng", "notes.txt"] {
            std::fs::write(directory.join(name), []).expect("fixture file must be created");
        }

        let sequence = list_dng_sequence(directory.join("frame2.DNG").display().to_string())
            .expect("directory DNGs must form a sequence");

        assert_eq!(
            sequence.file_names,
            ["frame1.dng", "frame2.DNG", "frame10.dng"]
        );
        std::fs::remove_dir_all(directory).expect("temporary directory must be removed");
    }

    #[test]
    fn dng_sequence_rejects_directory_without_dng_files() {
        let directory =
            std::env::temp_dir().join(format!("rime-empty-dng-sequence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).expect("temporary directory must be created");
        let selected = directory.join("notes.txt");
        std::fs::write(&selected, []).expect("fixture file must be created");

        let error = list_dng_sequence(selected.display().to_string())
            .expect_err("empty sequence must fail");

        assert!(error.starts_with("DNG_SEQUENCE_EMPTY:"));
        std::fs::remove_dir_all(directory).expect("temporary directory must be removed");
    }
}
