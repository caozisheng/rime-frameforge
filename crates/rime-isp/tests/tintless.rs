#![expect(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    reason = "fixed tintless fixture grids and exact identity gains"
)]

use rime_isp::{
    FrameIdentity, LCST_AVERAGE_VALUES, LCST_HISTOGRAM_VALUES, LcstStatisticsPacket,
    vbe::tintless::{
        TINTLESS_MESH_HEIGHT, TINTLESS_MESH_VALUES, TINTLESS_MESH_WIDTH, estimate_gain_mesh,
    },
};

const IDENTITY: FrameIdentity = FrameIdentity {
    frame_index: 4,
    run_revision: 2,
    method_revision: 3,
};

fn packet_with<F>(mut cell: F) -> LcstStatisticsPacket
where
    F: FnMut(usize, usize, usize) -> f32,
{
    let mut averages = vec![0.0; LCST_AVERAGE_VALUES];
    for y in 0..48 {
        for x in 0..64 {
            for channel in 0..4 {
                averages[(y * 64 + x) * 4 + channel] = cell(x, y, channel);
            }
        }
    }
    let mut histograms = Vec::with_capacity(LCST_HISTOGRAM_VALUES);
    for tile_y in 0..16 {
        let y0 = tile_y * 480 / 16;
        let y1 = (tile_y + 1) * 480 / 16;
        for tile_x in 0..16 {
            let x0 = tile_x * 640 / 16;
            let x1 = (tile_x + 1) * 640 / 16;
            let mut bins = [0_u32; 16];
            bins[0] = ((x1 - x0) * (y1 - y0)) as u32;
            histograms.extend(bins);
        }
    }
    LcstStatisticsPacket::new(IDENTITY, 640, 480, [0, 1, 1, 2], averages, histograms)
        .expect("valid LCST packet")
}

fn radius(x: usize, y: usize) -> f32 {
    let px = (x as f32 + 0.5) * 640.0 / 64.0;
    let py = (y as f32 + 0.5) * 480.0 / 48.0;
    let dx = px - 319.5;
    let dy = py - 239.5;
    let corner = (319.5_f32 * 319.5 + 239.5_f32 * 239.5).sqrt();
    (dx * dx + dy * dy).sqrt() / corner
}

#[test]
fn neutral_and_globally_colored_fields_keep_identity_mesh() {
    for colored in [false, true] {
        let packet = packet_with(|_, _, channel| match (colored, channel) {
            (false, _) => 0.25,
            (true, 0) => 0.40,
            (true, 1 | 2) => 0.20,
            (true, 3) => 0.60,
            _ => unreachable!(),
        });
        let mesh = estimate_gain_mesh(&packet, IDENTITY, [640, 480], [0, 1, 1, 2])
            .expect("qualified field must fit");
        assert_eq!(mesh.entries().len(), TINTLESS_MESH_VALUES);
        assert!(
            mesh.entries()
                .chunks_exact(2)
                .all(|gain| { (gain[0] - 1.0).abs() < 1e-4 && (gain[1] - 1.0).abs() < 1e-4 })
        );
    }
}

#[test]
fn radial_field_recovers_inverse_r_and_b_gains() {
    let radial_r = 0.30_f32;
    let radial_b = -0.20_f32;
    let packet = packet_with(|x, y, channel| {
        let g = 0.25;
        let r = radius(x, y);
        match channel {
            0 => g * (radial_r * r).exp(),
            1 | 2 => g,
            3 => g * (radial_b * r).exp(),
            _ => unreachable!(),
        }
    });
    let mesh = estimate_gain_mesh(&packet, IDENTITY, [640, 480], [0, 1, 1, 2])
        .expect("radial field must fit");
    let center = mesh.entry(TINTLESS_MESH_WIDTH / 2, TINTLESS_MESH_HEIGHT / 2);
    let corner = mesh.entry(0, 0);
    assert!((center[0] - 1.0).abs() < 0.01);
    assert!((center[1] - 1.0).abs() < 0.01);
    assert!(
        (corner[0] - (-radial_r).exp()).abs() < 0.03,
        "unexpected red corner gain: {}",
        corner[0]
    );
    assert!(
        (corner[1] - (-radial_b).exp()).abs() < 0.03,
        "unexpected blue corner gain: {}",
        corner[1]
    );
}

#[test]
fn concentric_scene_hues_are_not_misclassified_as_radial_shading() {
    let packet = packet_with(|x, y, channel| {
        let green = 0.25;
        let scene_hue = if radius(x, y) < 0.45 {
            0.03_f32
        } else {
            -0.03_f32
        };
        match channel {
            0 | 3 => green * scene_hue.exp(),
            1 | 2 => green,
            _ => unreachable!(),
        }
    });
    let mesh = estimate_gain_mesh(&packet, IDENTITY, [640, 480], [0, 1, 1, 2])
        .expect("separate scene hues must fit");

    assert!(
        mesh.entries().iter().all(|gain| (*gain - 1.0).abs() < 0.01),
        "scene hue boundaries must not become tintless gain"
    );
}

#[test]
fn method_revision_mismatch_returns_identity_error() {
    let packet = packet_with(|_, _, _| 0.25);
    let consumer = FrameIdentity {
        method_revision: IDENTITY.method_revision + 1,
        ..IDENTITY
    };
    let error = estimate_gain_mesh(&packet, consumer, [640, 480], [0, 1, 1, 2])
        .expect_err("stale-method statistics must fail");

    assert_eq!(
        error.to_string(),
        "tintless LCST producer/consumer frame identity is incompatible"
    );
}

#[test]
fn incompatible_packet_geometry_returns_stable_error() {
    let packet = packet_with(|_, _, _| 0.25);
    let error = estimate_gain_mesh(&packet, IDENTITY, [641, 480], [0, 1, 1, 2])
        .expect_err("mismatched source extent must fail");
    assert_eq!(
        error.to_string(),
        "tintless LCST source extent does not match the Bayer input"
    );
}

fn preprocess_context(statistics: Option<LcstStatisticsPacket>) -> rime_isp::PreprocessContext {
    rime_isp::PreprocessContext {
        identity: IDENTITY,
        width: 640,
        height: 480,
        black_level: 0.0,
        white_level: 1.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: None,
        as_shot_white_xy: None,
        color_matrix1: [0.0; 9],
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
        lcst_statistics: statistics,
        drc_local_cold_start: false,
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
        vignette_radial: Vec::new(),
        gain_maps: Vec::new(),
    }
}

#[test]
fn tintless_preprocess_freezes_mesh_uniform_and_storage() {
    use rime_isp::Operator as _;

    let packet = packet_with(|x, y, channel| {
        let green = 0.25;
        let radial = radius(x, y);
        match channel {
            0 => green * (0.2 * radial).exp(),
            1 | 2 => green,
            3 => green * (-0.1 * radial).exp(),
            _ => unreachable!(),
        }
    });
    let frozen = rime_isp::vbe::tintless::OPERATOR
        .preprocess("00", &preprocess_context(Some(packet)))
        .expect("tintless packet");

    assert_eq!(frozen.bytes().len(), 48);
    let mesh = frozen.resource("gain_mesh").expect("gain mesh resource");
    assert_eq!(mesh.extent(), [65, 49, 2]);
    assert_eq!(mesh.bytes().len(), TINTLESS_MESH_VALUES * size_of::<f32>());
}

#[test]
fn tintless_cold_start_freezes_identity_mesh() {
    use rime_isp::Operator as _;

    let frozen = rime_isp::vbe::tintless::OPERATOR
        .preprocess("00", &preprocess_context(None))
        .expect("cold-start identity packet");
    let mesh = frozen.resource("gain_mesh").expect("gain mesh resource");
    assert!(
        mesh.bytes()
            .chunks_exact(4)
            .all(|bytes| { f32::from_ne_bytes(bytes.try_into().expect("f32 mesh value")) == 1.0 })
    );
}
