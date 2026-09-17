use std::path::Path;

use gamut_dng::{DngRewrite, Opcode, OpcodeList, Value, opcode_id, tags};

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
fn warp_rectilinear_is_typed_and_changes_metadata_identity() {
    let data = std::fs::read(GH5S_SAMPLE).expect("GH5S fixture must exist");
    let baseline = DngReader::new()
        .decode_bytes(Path::new("baseline.dng"), &data, 0)
        .expect("baseline DNG must decode");
    let mut parameters = Vec::new();
    parameters.extend_from_slice(&3_u32.to_be_bytes());
    for coefficients in [
        [1.0_f64, 0.01, 0.001, 0.0, 0.0, 0.0],
        [1.0_f64, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0_f64, -0.01, -0.001, 0.0, 0.0, 0.0],
    ] {
        for coefficient in coefficients {
            parameters.extend_from_slice(&coefficient.to_be_bytes());
        }
    }
    parameters.extend_from_slice(&0.49_f64.to_be_bytes());
    parameters.extend_from_slice(&0.51_f64.to_be_bytes());
    let mut opcodes = OpcodeList::new();
    opcodes.push(Opcode {
        id: opcode_id::WARP_RECTILINEAR,
        spec_version: [1, 3, 0, 0],
        flags: Opcode::FLAG_OPTIONAL,
        parameters,
    });
    let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
    rewrite
        .file_mut()
        .ifds
        .first_mut()
        .expect("IFD0")
        .sub_ifds_mut()
        .first_mut()
        .expect("raw SubIFD group")
        .ifds
        .first_mut()
        .expect("raw IFD")
        .set(tags::OPCODE_LIST3, Value::Undefined(opcodes.to_bytes()));
    let warped_bytes = rewrite.write().expect("warp fixture rewrite").bytes;

    let warped = DngReader::new()
        .decode_bytes(Path::new("warped.dng"), &warped_bytes, 0)
        .expect("WarpRectilinear DNG must decode");

    assert_eq!(warped.metadata.warp_rectilinear.len(), 1);
    assert_eq!(
        warped.metadata.warp_rectilinear[0].coefficient_sets[0]
            .radial
            .map(f64::to_bits),
        [1.0, 0.01, 0.001, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        warped.metadata.warp_rectilinear[0]
            .optical_center
            .map(f64::to_bits),
        [0.49, 0.51].map(f64::to_bits)
    );
    assert_ne!(
        warped.metadata.metadata_hash,
        baseline.metadata.metadata_hash
    );
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
fn gain_map_opcode_decodes_typed() {
    let data = std::fs::read(GH5S_SAMPLE).expect("GH5S fixture must exist");
    let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
    rewrite
        .file_mut()
        .ifds
        .first_mut()
        .expect("IFD0")
        .sub_ifds_mut()
        .first_mut()
        .expect("raw SubIFD group")
        .ifds
        .first_mut()
        .expect("raw IFD")
        .set(tags::OPCODE_LIST2, Value::Undefined(gain_map_opcodes()));
    let bytes = rewrite.write().expect("gain map rewrite").bytes;

    let frame = DngReader::new()
        .decode_bytes(Path::new("gain-map.dng"), &bytes, 0)
        .expect("GainMap DNG must decode");

    assert_eq!(frame.metadata.gain_map.len(), 1);
    let gain_map = &frame.metadata.gain_map[0];
    assert_eq!(gain_map.spec_version, [1, 3, 0, 0]);
    assert_eq!(gain_map.flags, 0);
    assert_eq!(gain_map.area, [0, 0, 2776, 3744]);
    assert_eq!(gain_map.first_plane, 0);
    assert_eq!(gain_map.plane_count, 1);
    assert_eq!(gain_map.row_pitch, 1);
    assert_eq!(gain_map.col_pitch, 1);
    assert_eq!(gain_map.points, [3, 5]);
    assert_eq!(
        gain_map.spacing.map(f64::to_bits),
        [0.25, 0.5].map(f64::to_bits)
    );
    assert_eq!(
        gain_map.origin.map(f64::to_bits),
        [0.1, 0.05].map(f64::to_bits)
    );
    assert_eq!(gain_map.map_planes, 2);
    assert_eq!(gain_map.entries.len(), 3 * 5 * 2);
    for (index, entry) in gain_map.entries.iter().enumerate() {
        assert_eq!(entry.to_bits(), expected_entry(index).to_bits());
    }
}

#[test]
fn rejects_malformed_gain_map() {
    let data = std::fs::read(GH5S_SAMPLE).expect("GH5S fixture must exist");
    let cases: &[(&str, Vec<u8>)] = &[
        ("truncated-entries", gain_map_parameters([3, 5], [0.25, 0.5], [0.1, 0.05], 2, &[])),
        ("points-zero", gain_map_parameters([0, 5], [0.25, 0.5], [0.1, 0.05], 2, &[])),
        ("spacing-zero", gain_map_parameters([3, 5], [0.0, 0.5], [0.1, 0.05], 2, &[1.0; 30])),
        ("nan-entry", gain_map_parameters([3, 5], [0.25, 0.5], [0.1, 0.05], 2, &nan_entries())),
        ("map-planes-zero", gain_map_parameters([3, 5], [0.25, 0.5], [0.1, 0.05], 0, &[])),
    ];
    for (label, parameters) in cases {
        let mut rewrite = DngRewrite::open(&data).expect("fixture must be rewriteable");
        rewrite
            .file_mut()
            .ifds
            .first_mut()
            .expect("IFD0")
            .sub_ifds_mut()
            .first_mut()
            .expect("raw SubIFD group")
            .ifds
            .first_mut()
            .expect("raw IFD")
            .set(tags::OPCODE_LIST2, Value::Undefined(gain_map_opcodes_bytes(parameters.clone())));
        let bytes = rewrite.write().expect("malformed rewrite").bytes;
        let error = DngReader::new()
            .decode_bytes(Path::new("malformed-gain-map.dng"), &bytes, 0)
            .expect_err("malformed GainMap must be rejected");
        assert!(
            matches!(error, DngReaderError::InvalidGainMap { .. }),
            "{label}: wrong error {error:?}"
        );
    }
}

#[test]
fn fixture_has_no_gain_map() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S DNG must decode");
    assert!(frame.metadata.gain_map.is_empty());
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

fn gain_map_opcodes() -> Vec<u8> {
    gain_map_opcodes_bytes(gain_map_parameters(
        [3, 5],
        [0.25, 0.5],
        [0.1, 0.05],
        2,
        &expected_entries(30),
    ))
}

fn gain_map_opcodes_bytes(parameters: Vec<u8>) -> Vec<u8> {
    let mut opcodes = OpcodeList::new();
    opcodes.push(Opcode {
        id: opcode_id::GAIN_MAP,
        spec_version: [1, 3, 0, 0],
        flags: 0,
        parameters,
    });
    opcodes.to_bytes()
}

fn gain_map_parameters(
    points: [u32; 2],
    spacing: [f64; 2],
    origin: [f64; 2],
    map_planes: u32,
    entries: &[f32],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    // area_spec: t, l, b, r, plane, planes, rowPitch, colPitch (Adobe DNG SDK dng_area_spec).
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&0_i32.to_be_bytes());
    bytes.extend_from_slice(&2776_i32.to_be_bytes());
    bytes.extend_from_slice(&3744_i32.to_be_bytes());
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&points[0].to_be_bytes());
    bytes.extend_from_slice(&points[1].to_be_bytes());
    bytes.extend_from_slice(&spacing[0].to_be_bytes());
    bytes.extend_from_slice(&spacing[1].to_be_bytes());
    bytes.extend_from_slice(&origin[0].to_be_bytes());
    bytes.extend_from_slice(&origin[1].to_be_bytes());
    bytes.extend_from_slice(&map_planes.to_be_bytes());
    for entry in entries {
        bytes.extend_from_slice(&entry.to_be_bytes());
    }
    bytes
}

fn expected_entries(count: usize) -> Vec<f32> {
    (0..count).map(expected_entry).collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "test helper generates small sequential f32 values by design"
)]
fn expected_entry(index: usize) -> f32 {
    1.0 + index as f32 * 0.01
}

fn nan_entries() -> Vec<f32> {
    vec![1.0; 29].into_iter().chain([f32::NAN]).collect()
}
