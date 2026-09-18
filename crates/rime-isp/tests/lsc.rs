use rime_isp::GainMapParameters;
use rime_isp::vbe::lsc::{MeshGeometry, VignetteRadialParameters, mesh_gain};
use rime_isp::{FrameIdentity, PreprocessContext};

fn context(
    opcodes: Vec<VignetteRadialParameters>,
    meshes: Vec<GainMapParameters>,
) -> PreprocessContext {
    PreprocessContext {
        identity: FrameIdentity {
            frame_index: 7,
            run_revision: 2,
            method_revision: 3,
        },
        width: 4,
        height: 3,
        black_level: 64.0,
        white_level: 4095.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: None,
        as_shot_white_xy: None,
        color_matrix1: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        color_matrix2: None,
        calibration_illuminant1_code: None,
        calibration_illuminant2_code: None,
        camera_calibration1: None,
        camera_calibration2: None,
        camera_calibration_signature: None,
        profile_calibration_signature: None,
        profile_hue_sat_map_dims: None,
        profile_hue_sat_map_data1: None,
        profile_hue_sat_map_data2: None,
        analog_balance: None,
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
        baseline_exposure_ev: None,
        exposure_time_seconds: None,
        f_number: None,
        drc_local_statistics: None,
        drc_exposure_policy: rime_isp::vbe::drc::DrcExposurePolicy::Baseline,
        drc_metered_target_ev100: None,
        drc_profile_adjustment_ev: 0.0,
        drc_gain_offset_ev: None,
        drc_knee: None,
        drc_amplifier: None,
        drc_modulation_curves: None,
        wbc_highlight_recovery: false,
        wbc_hr_gain: None,
        drc_details_amplify: true,
        dem_thresholds: None,
        vignette_radial: opcodes,
        gain_maps: meshes,
    }
}

fn empty_mesh() -> GainMapParameters {
    GainMapParameters {
        points: [2, 3],
        spacing: [0.5, 0.25],
        origin: [0.0, 0.0],
        planes: 1,
        area: [0, 0, 0, 0],
        row_pitch: 1,
        col_pitch: 1,
        entries: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_ne_bytes(bytes[offset..offset + 4].try_into().expect("f32 field"))
}

fn preprocess(
    opcodes: Vec<VignetteRadialParameters>,
    meshes: Vec<GainMapParameters>,
) -> rime_isp::ModuleParameterPacket {
    rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess("00", &context(opcodes, meshes))
        .expect("LSC preprocessing")
}

#[expect(dead_code, reason = "kept for the upcoming composition tests")]
fn constant_vignette(gain: f64) -> VignetteRadialParameters {
    VignetteRadialParameters {
        coefficients: [gain, 0.0, 0.0, 0.0, 0.0],
        optical_center: [0.5, 0.5],
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "test helper generates small sequential f32 values by design"
)]
fn expected_entries(count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| 1.0 + (index as f32) * 0.01)
        .collect()
}

#[test]
fn lsc_preprocess_freezes_gain_map_meshes() {
    // One GainMap opcode passes through byte-faithfully: header geometry
    // and every entry survive the packet round-trip unchanged.
    let mesh = GainMapParameters {
        points: [3, 5],
        spacing: [0.25, 0.5],
        origin: [0.1, 0.05],
        planes: 2,
        area: [0, 0, 0, 0],
        row_pitch: 1,
        col_pitch: 1,
        entries: expected_entries(3 * 5 * 2),
    };
    let packet = preprocess(Vec::new(), vec![mesh]);

    assert_eq!(u32_at(packet.bytes(), 0), 1);
    let headers = packet.resource("gain_mesh_headers").expect("headers");
    let header = headers.bytes();
    assert_eq!(u32_at(header, 0), 3);
    assert_eq!(u32_at(header, 4), 5);
    assert_eq!(u32_at(header, 8), 2);
    assert_eq!(f32_at(header, 16).to_bits(), 0.25_f32.to_bits());
    assert_eq!(f32_at(header, 20).to_bits(), 0.5_f32.to_bits());
    assert_eq!(f32_at(header, 24).to_bits(), 0.1_f32.to_bits());
    // All-zero area spec normalizes to the whole 3(h)×4(w) frame:
    // [top, left, bottom, right] = [0, 0, 3, 4] at header bytes 32..48;
    // unpitched 1×1 application grid at bytes 48..56.
    assert_eq!(u32_at(header, 32), 0);
    assert_eq!(u32_at(header, 36), 0);
    assert_eq!(u32_at(header, 40), 3);
    assert_eq!(u32_at(header, 44), 4);
    assert_eq!(u32_at(header, 48), 1);
    assert_eq!(u32_at(header, 52), 1);
    assert_eq!(u32_at(header, 12), 0);
    let entries = packet.resource("gain_mesh_entries").expect("entries");
    for (index, expected) in expected_entries(3 * 5 * 2).into_iter().enumerate() {
        assert_eq!(
            f32_at(entries.bytes(), index * 4).to_bits(),
            expected.to_bits(),
            "entry {index}"
        );
    }
}

#[test]
fn lsc_preprocess_normalizes_open_ended_area_specs() {
    // DNG `dng_area_spec::ScaledOverlap` semantics: an empty spec covers
    // the whole image. `[1, 1, 0, 0]` is empty (bottom <= top), so it
    // becomes the full 3(h)×4(w) frame `[0, 0, 3, 4]`, while an explicit
    // non-empty spec intersects with the frame bounds — `[2, 2, 9, 9]`
    // clamps to `[2, 2, 3, 4]`.
    let mesh = GainMapParameters {
        points: [2, 2],
        spacing: [1.0, 1.0],
        origin: [0.0, 0.0],
        planes: 1,
        area: [1, 1, 0, 0],
        row_pitch: 1,
        col_pitch: 1,
        entries: vec![1.0; 4],
    };
    let packet = preprocess(Vec::new(), vec![mesh]);
    let header = packet
        .resource("gain_mesh_headers")
        .expect("headers")
        .bytes();
    assert_eq!(u32_at(header, 32), 0);
    assert_eq!(u32_at(header, 36), 0);
    assert_eq!(u32_at(header, 40), 3);
    assert_eq!(u32_at(header, 44), 4);

    let mesh = GainMapParameters {
        points: [2, 2],
        spacing: [1.0, 1.0],
        origin: [0.0, 0.0],
        planes: 1,
        area: [2, 2, 9, 9],
        row_pitch: 1,
        col_pitch: 1,
        entries: vec![1.0; 4],
    };
    let packet = preprocess(Vec::new(), vec![mesh]);
    let header = packet
        .resource("gain_mesh_headers")
        .expect("headers")
        .bytes();
    assert_eq!(u32_at(header, 32), 2);
    assert_eq!(u32_at(header, 36), 2);
    assert_eq!(u32_at(header, 40), 3);
    assert_eq!(u32_at(header, 44), 4);
}

/// Test-local oracle: replicates the rasterization math (node → pixel →
/// gain) with its own arithmetic, so geometry/serialization bugs in
/// `lsc_common` cannot hide behind a circular call.
fn expected_vignette_entry(row: u32, col: u32) -> f32 {
    let extent = [4.0_f32, 3.0_f32];
    let center = [0.5_f32 * extent[0], 0.5_f32 * extent[1]];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "test oracle mirrors the production f32 rasterization"
    )]
    let pixel_x = ((f64::from(col) / 32.0) * f64::from(extent[0])) as f32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "test oracle mirrors the production f32 rasterization"
    )]
    let pixel_y = ((f64::from(row) / 32.0) * f64::from(extent[1])) as f32;
    let pixel = [pixel_x, pixel_y];
    let delta = [pixel[0] - center[0], pixel[1] - center[1]];
    let farthest = [
        center[0].max(extent[0] - center[0]),
        center[1].max(extent[1] - center[1]),
    ];
    let maximum_radius_squared =
        (farthest[0] * farthest[0] + farthest[1] * farthest[1]).max(1.0e-12);
    let radius_squared = (delta[0] * delta[0] + delta[1] * delta[1]) / maximum_radius_squared;
    1.0 + radius_squared * 0.5
}

#[test]
fn lsc_gain_map_honors_pitch_lattice_and_axis_order() {
    let mesh = GainMapParameters {
        points: [2, 3],
        spacing: [0.5, 0.25],
        origin: [0.0, 0.0],
        planes: 1,
        area: [0, 1, 3, 4],
        row_pitch: 2,
        col_pitch: 2,
        entries: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
    };
    let packet = preprocess(Vec::new(), vec![mesh]);
    let header = packet
        .resource("gain_mesh_headers")
        .expect("headers")
        .bytes();
    assert_eq!(header.len(), 56);
    assert_eq!(u32_at(header, 48), 2);
    assert_eq!(u32_at(header, 52), 2);

    let geometry = MeshGeometry {
        points: [2, 3],
        spacing: [0.5, 0.25],
        origin: [0.0, 0.0],
        planes: 1,
        area: [0, 1, 3, 4],
        row_pitch: 2,
        col_pitch: 2,
    };
    let entries = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let gain = mesh_gain(geometry, &entries, 0, 0, [1, 0], [8.0, 4.0]);
    assert!(
        (gain - 2.5).abs() <= f32::EPSILON,
        "wrong pitched gain {gain}"
    );
    assert_eq!(
        mesh_gain(geometry, &entries, 0, 0, [2, 0], [8.0, 4.0]).to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        mesh_gain(geometry, &entries, 0, 0, [1, 1], [8.0, 4.0]).to_bits(),
        1.0_f32.to_bits()
    );
}

#[test]
fn lsc_vignette_mesh_matches_polynomial() {
    // k0 = 0.5 → gain(node) = 1 + 0.5·r²(node); every node position must
    // hit the polynomial exactly.
    let packet = preprocess(
        vec![VignetteRadialParameters {
            coefficients: [0.5, 0.0, 0.0, 0.0, 0.0],
            optical_center: [0.5, 0.5],
        }],
        Vec::new(),
    );

    assert_eq!(u32_at(packet.bytes(), 0), 1);
    let headers = packet.resource("gain_mesh_headers").expect("headers");
    let header = headers.bytes();
    assert_eq!(u32_at(header, 0), 33);
    assert_eq!(u32_at(header, 4), 33);
    assert_eq!(u32_at(header, 8), 1);
    assert_eq!(f32_at(header, 16).to_bits(), (1.0_f32 / 32.0).to_bits());
    assert_eq!(f32_at(header, 24).to_bits(), 0.0_f32.to_bits());

    let entries = packet.resource("gain_mesh_entries").expect("entries");
    assert_eq!(entries.extent(), [33 * 33, 1, 1]);
    for row in 0..33_u32 {
        for col in 0..33_u32 {
            let index = (row * 33 + col) as usize;
            let value = f32_at(entries.bytes(), index * 4);
            assert_eq!(
                value.to_bits(),
                expected_vignette_entry(row, col).to_bits(),
                "node ({row}, {col})"
            );
        }
    }
}

#[test]
fn lsc_preprocess_multiplies_sources_in_order() {
    let packet = preprocess(
        vec![VignetteRadialParameters {
            coefficients: [0.5, 0.0, 0.0, 0.0, 0.0],
            optical_center: [0.5, 0.5],
        }],
        vec![empty_mesh()],
    );

    assert_eq!(u32_at(packet.bytes(), 0), 2);
    let entries = packet.resource("gain_mesh_entries").expect("entries");
    // Passthrough GainMap entries first, then the 33x33 rasterized mesh.
    let passthrough = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0];
    for (index, value) in passthrough.into_iter().enumerate() {
        assert_eq!(
            f32_at(entries.bytes(), index * 4).to_bits(),
            value.to_bits()
        );
    }
    let raster = &entries.bytes()[passthrough.len() * 4..];
    for row in 0..33_u32 {
        for col in 0..33_u32 {
            let index = (row * 33 + col) as usize;
            let value = f32_at(raster, index * 4);
            assert_eq!(
                value.to_bits(),
                expected_vignette_entry(row, col).to_bits(),
                "node ({row}, {col})"
            );
        }
    }
    let headers = packet.resource("gain_mesh_headers").expect("headers");
    // Second header's entries_offset counts f32 units from the buffer start.
    assert_eq!(u32_at(headers.bytes(), 56 + 12), 6);
}

#[test]
fn lsc_preprocess_uses_bypass_packet_when_no_sources() {
    let packet = preprocess(Vec::new(), Vec::new());

    assert_eq!(u32_at(packet.bytes(), 0), 0);
    let headers = packet.resource("gain_mesh_headers").expect("headers");
    assert_eq!(headers.extent(), [1, 1, 1]);
    assert!(headers.bytes().iter().all(|byte| *byte == 0));
    let entries = packet.resource("gain_mesh_entries").expect("entries");
    assert_eq!(entries.extent(), [1, 1, 1]);
    assert!(entries.bytes().iter().all(|byte| *byte == 0));
}

#[test]
fn lsc_preprocess_rejects_invalid_gain_map_mesh() {
    let cases: Vec<(&str, GainMapParameters)> = vec![
        (
            "points-zero",
            GainMapParameters {
                points: [0, 3],
                spacing: [0.5, 0.25],
                origin: [0.0, 0.0],
                planes: 1,
                area: [0, 0, 0, 0],
                row_pitch: 1,
                col_pitch: 1,
                entries: vec![1.0; 6],
            },
        ),
        (
            "spacing-zero",
            GainMapParameters {
                points: [2, 3],
                spacing: [0.0, 0.25],
                origin: [0.0, 0.0],
                planes: 1,
                area: [0, 0, 0, 0],
                row_pitch: 1,
                col_pitch: 1,
                entries: vec![1.0; 6],
            },
        ),
        (
            "planes-zero",
            GainMapParameters {
                points: [2, 3],
                spacing: [0.5, 0.25],
                origin: [0.0, 0.0],
                planes: 0,
                area: [0, 0, 0, 0],
                row_pitch: 1,
                col_pitch: 1,
                entries: vec![1.0; 6],
            },
        ),
        (
            "length-mismatch",
            GainMapParameters {
                points: [2, 3],
                spacing: [0.5, 0.25],
                origin: [0.0, 0.0],
                planes: 1,
                area: [0, 0, 0, 0],
                row_pitch: 1,
                col_pitch: 1,
                entries: vec![1.0; 5],
            },
        ),
        (
            "nan-entry",
            GainMapParameters {
                points: [2, 3],
                spacing: [0.5, 0.25],
                origin: [0.0, 0.0],
                planes: 1,
                area: [0, 0, 0, 0],
                row_pitch: 1,
                col_pitch: 1,
                entries: vec![1.0, f32::NAN, 3.0, 4.0, 5.0, 6.0],
            },
        ),
    ];
    for (label, mesh) in cases {
        let error = rime_isp::operator_by_id("lsc")
            .expect("LSC")
            .preprocess("00", &context(Vec::new(), vec![mesh]))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("operator `lsc` preprocessing failed"),
            "{label}: wrong error {error}"
        );
    }
}

#[test]
fn lsc_preprocess_rejects_values_outside_gpu_range() {
    let error = rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess(
            "00",
            &context(
                vec![VignetteRadialParameters {
                    coefficients: [f64::MAX, 0.0, 0.0, 0.0, 0.0],
                    optical_center: [0.5, 0.5],
                }],
                Vec::new(),
            ),
        )
        .expect_err("finite f64 values that overflow f32 must fail");

    assert_eq!(
        error.to_string(),
        "operator `lsc` preprocessing failed: FixVignetteRadial values must be finite, GPU-representable, and not overflow f32",
    );
}

#[test]
fn lsc_preprocess_rejects_mesh_geometry_outside_gpu_range() {
    let error = rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess(
            "00",
            &context(
                Vec::new(),
                vec![GainMapParameters {
                    points: [2, 3],
                    spacing: [f64::MAX, 0.25],
                    origin: [0.0, 0.0],
                    planes: 1,
                    area: [0, 0, 0, 0],
                    row_pitch: 1,
                    col_pitch: 1,
                    entries: vec![1.0; 6],
                }],
            ),
        )
        .expect_err("finite f64 mesh spacing that overflows f32 must fail");

    assert!(
        error
            .to_string()
            .contains("operator `lsc` preprocessing failed"),
        "wrong error {error}"
    );
}
