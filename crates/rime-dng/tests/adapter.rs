use std::path::Path;

use rime_dng::{BayerCfa, DngReader, DngReaderError, RawFrameLayout};

const GH5S_SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../pipeline/normal/P1020601.dng");

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
fn decoded_samples_match_declared_layout() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S DNG must decode");

    assert_eq!(
        frame.samples.len(),
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
