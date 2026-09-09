#![expect(
    dead_code,
    reason = "the command module is included to exercise descriptor serialization"
)]

#[path = "../src/dng_command.rs"]
mod dng_command;

use std::path::Path;

use rime_dng::DngReader;

const GH5S: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../pipeline/normal/P1020601.dng"
);

#[test]
fn descriptor_serializes_complete_metadata_and_filename() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 0)
        .expect("GH5S frame must decode");
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new(GH5S))
        .expect("descriptor must preprocess white balance");
    let json = serde_json::to_value(descriptor).expect("descriptor serializes");

    assert_eq!(json["fileName"], "P1020601.dng");
    assert_eq!(
        json["metadata"]["colorMatrix1"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        json["metadata"]["asShotNeutral"].as_array().map(Vec::len),
        Some(3)
    );
    assert!(json["metadata"]["asShotWhiteXY"].is_null());
    assert_eq!(json["whiteBalanceGains"].as_array().map(Vec::len), Some(3));
    assert!(
        json["metadata"]["ifd0Extra"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty())
    );
    assert!(
        json["metadata"]["rawExtra"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty())
    );
}

#[test]
fn descriptor_serializes_solved_color_reproduce_assets() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 3)
        .expect("GH5S frame must decode");
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new(GH5S))
        .expect("descriptor must preprocess color reproduce");
    let json = serde_json::to_value(&descriptor).expect("descriptor serializes");

    let cr = &json["colorReproduce"];
    assert!(
        cr.is_object(),
        "colorReproduce assets must be present for the GH5S frame"
    );
    // Matrices match the design-doc golden values (1e-4 over JSON f32).
    let sensor = cr["sensorToProphoto"].as_array().expect("matrix present");
    assert_eq!(sensor.len(), 9);
    assert!((sensor[0].as_f64().expect("f64") - 0.759_738).abs() < 1e-4);
    assert!((sensor[4].as_f64().expect("f64") - 1.300_616).abs() < 1e-4);
    let to_srgb = cr["prophotoToSrgb"].as_array().expect("matrix present");
    assert_eq!(to_srgb.len(), 9);
    assert!((to_srgb[0].as_f64().expect("f64") - 2.036_832).abs() < 1e-4);
    // HS calibration: dims (90, 30), interpolated table of 8100 floats
    // (ValueDivs == 1: H x S grid, v layer constant).
    assert_eq!(
        cr["hsDims"]
            .as_array()
            .map(|dims| (dims[0].as_u64(), dims[1].as_u64())),
        Some((Some(90), Some(30)))
    );
    let lut = cr["hsLut"].as_array().expect("interpolated LUT present");
    assert_eq!(lut.len(), 8100);
    assert_eq!(cr["hsEnable"].as_bool(), Some(true));
}
