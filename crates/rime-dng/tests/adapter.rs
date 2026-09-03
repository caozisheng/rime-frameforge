use std::path::Path;

use gamut_dng::{DngRewrite, Value, tags};

use rime_dng::{BayerCfa, DngReader, DngReaderError, RawFrameLayout};

const GH5S_SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../pipeline/normal/P1020601.dng"
);

#[test]
fn gh5s_sample_decodes_as_bayer_raw() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S DNG must decode");

    assert!(frame.layout.width > 0);
    assert!(frame.layout.height > 0);
    assert!(matches!(
        frame.layout.cfa,
        BayerCfa::Rggb | BayerCfa::Grbg | BayerCfa::Gbrg | BayerCfa::Bggr
    ));
}

#[test]
fn pixel_white_xy_dng_decodes_without_as_shot_neutral() {
    let data = std::fs::read(GH5S_SAMPLE).expect("GH5S fixture must exist");
    let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
    let ifd0 = rewrite.file_mut().ifds.first_mut().expect("IFD0");
    ifd0.remove(tags::AS_SHOT_NEUTRAL);
    ifd0.set(
        tags::AS_SHOT_WHITE_XY,
        Value::Rational(vec![(1, 4), (1, 4)]),
    );
    let white_xy = rewrite.write().expect("WhiteXY fixture rewrite").bytes;

    let frame = DngReader::new()
        .decode_bytes(Path::new("pixel-white-xy.dng"), &white_xy, 0)
        .expect("WhiteXY-only DNG must decode");

    assert_eq!(frame.metadata.as_shot_neutral, None);
    assert_eq!(frame.metadata.as_shot_white_xy, Some([0.25, 0.25]));
    assert!(
        frame
            .metadata
            .ifd0_extra
            .iter()
            .all(|tag| tag.tag != tags::AS_SHOT_WHITE_XY)
    );
}

#[test]
fn malformed_as_shot_neutral_does_not_fall_back_to_white_xy() {
    let data = std::fs::read(GH5S_SAMPLE).expect("GH5S fixture must exist");
    let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
    let ifd0 = rewrite.file_mut().ifds.first_mut().expect("IFD0");
    ifd0.set(tags::AS_SHOT_NEUTRAL, Value::Rational(vec![(1, 2), (1, 1)]));
    ifd0.set(
        tags::AS_SHOT_WHITE_XY,
        Value::Rational(vec![(1, 4), (1, 4)]),
    );
    let malformed = rewrite.write().expect("malformed fixture rewrite").bytes;

    let error = DngReader::new()
        .decode_bytes(Path::new("malformed-neutral.dng"), &malformed, 0)
        .expect_err("malformed AsShotNeutral must not fall back");

    assert!(error.to_string().contains("malformed AsShotNeutral"));
}

#[test]
fn decoded_samples_match_declared_layout() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S DNG must decode");

    assert_eq!(
        frame.samples().len(),
        (frame.layout.row_stride_samples * frame.layout.height) as usize
    );
}

#[test]
fn decoded_storage_bits_are_supported() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S DNG must decode");

    assert!((1..=16).contains(&frame.layout.storage_bits));
}

#[test]
fn unsupported_photometry_has_a_stable_error() {
    let error = DngReader::validate_layout(&RawFrameLayout {
        width: 2,
        height: 2,
        row_stride_samples: 4,
        storage_bits: 12,
        cfa: BayerCfa::Unsupported,
    })
    .expect_err("unsupported CFA must fail");

    assert!(matches!(error, DngReaderError::UnsupportedPhotometry));
}

#[test]
fn dng_1_3_is_rejected_but_dng_1_4_is_accepted_by_version_gate() {
    assert!(DngReader::validate_version([1, 3, 0, 0]).is_err());
    assert!(DngReader::validate_version([1, 4, 0, 0]).is_ok());
}
