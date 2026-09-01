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
    let descriptor = dng_command::descriptor_from_frame(&frame, Path::new(GH5S));
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
