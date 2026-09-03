use std::path::Path;

use rime_dng::DngReader;

const GH5S: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../pipeline/normal/P1020601.dng"
);

#[test]
fn decoded_frame_exposes_complete_inspector_metadata() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S), 12)
        .expect("GH5S frame must decode");

    assert_eq!(frame.frame_index, 12);
    assert_eq!(frame.metadata.color_matrix1.len(), 9);
    assert!(frame.metadata.as_shot_neutral.is_some_and(|neutral| {
        neutral
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
    }));
    assert_eq!(frame.metadata.as_shot_white_xy, None);
    assert_eq!(frame.metadata.black_repeat, (2, 2));
    assert_eq!(frame.metadata.black_levels.len(), 4);
    assert!(!frame.metadata.ifd0_extra.is_empty());
    assert!(!frame.metadata.raw_extra.is_empty());
}
