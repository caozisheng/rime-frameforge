use std::path::Path;

use rime_dng::DngReader;

const GH5S: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../pipeline/normal/P1020601.dng"
);

#[test]
fn decoded_frame_exposes_color_reproduce_metadata() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 0)
        .expect("GH5S frame must decode");
    let metadata = &frame.metadata;
    // Panasonic writes the illuminants as SHORT codes; gamut-dng reads them
    // through its Panasonic-compatible tag slots (StdA=17, D65=21).
    assert_eq!(metadata.calibration_illuminant1_code, Some(17));
    assert_eq!(metadata.calibration_illuminant2_code, Some(21));
    // No CameraCalibrationSignature in this file: the MATLAB reference falls
    // back to identity calibration matrices.
    assert_eq!(metadata.camera_calibration_signature, None);
    assert_eq!(
        metadata.profile_calibration_signature.as_deref(),
        Some("com.adobe")
    );

    // HSV calibration: dims (90, 30, 1), two 8100-entry float tables.
    assert_eq!(metadata.profile_hue_sat_map_dims, Some([90, 30, 1]));
    let data1 = metadata
        .profile_hue_sat_map_data1
        .as_ref()
        .expect("ProfileHueSatMapData1 present");
    let data2 = metadata
        .profile_hue_sat_map_data2
        .as_ref()
        .expect("ProfileHueSatMapData2 present");
    assert_eq!(data1.len(), 8100);
    assert_eq!(data2.len(), 8100);
    assert!(
        data1
            .iter()
            .chain(data2.iter())
            .all(|value| value.is_finite())
    );
}
