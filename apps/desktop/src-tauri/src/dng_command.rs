use std::path::Path;

use rime_dng::DngReader;
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
    pub as_shot_neutral: [f64; 3],
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
    pub metadata: DngMetadataDescriptor,
}

#[tauri::command]
pub fn inspect_dng_frame(
    path: String,
    frame_index: Option<u64>,
) -> Result<DngFrameDescriptor, String> {
    let frame = DngReader::new()
        .decode_file(Path::new(&path), frame_index.unwrap_or(0))
        .map_err(|error| error.to_string())?;
    Ok(descriptor_from_frame(&frame, Path::new(&path)))
}

#[tauri::command]
pub fn read_dng_raw(path: String) -> Result<Response, String> {
    let frame = DngReader::new()
        .decode_file(Path::new(&path), 0)
        .map_err(|error| error.to_string())?;
    Ok(Response::new(raw_samples_le(&frame)))
}

fn raw_samples_le(frame: &rime_dng::DecodedRawFrame) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(frame.samples.len() * 2);
    for sample in &frame.samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

pub fn descriptor_from_frame(frame: &rime_dng::DecodedRawFrame, path: &Path) -> DngFrameDescriptor {
    let metadata = &frame.metadata;
    DngFrameDescriptor {
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
        metadata: metadata_descriptor(metadata),
    }
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
    const GH5S: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../pipeline/normal/P1020601.dng");
    #[test]
    fn gh5s_descriptor_and_binary_length_match() {
        let frame = DngReader::new()
            .decode_file(Path::new(GH5S), 0)
            .expect("GH5S frame must decode");
        let descriptor = descriptor_from_frame(&frame, Path::new(GH5S));
        let bytes = raw_samples_le(&frame);
        assert_eq!(descriptor.width, 3744);
        assert_eq!(descriptor.height, 2776);
        assert_eq!(bytes.len(), frame.samples.len() * 2);
        assert!(!descriptor.metadata.raw_extra.is_empty());
    }
}
