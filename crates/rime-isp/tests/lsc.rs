use rime_isp::vbe::lsc::VignetteRadialParameters;
use rime_isp::{FrameIdentity, PreprocessContext};

fn context(opcodes: Vec<VignetteRadialParameters>) -> PreprocessContext {
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
    }
}

#[test]
fn lsc_preprocess_freezes_vignette_records_in_order() {
    let packet = rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess(
            "00",
            &context(vec![
                VignetteRadialParameters {
                    coefficients: [1.0, 0.1, 0.01, 0.001, 0.0001],
                    optical_center: [0.45, 0.55],
                },
                VignetteRadialParameters {
                    coefficients: [0.9, 0.2, 0.02, 0.002, 0.0002],
                    optical_center: [0.4, 0.6],
                },
            ]),
        )
        .expect("LSC preprocessing");

    assert_eq!(
        u32::from_ne_bytes(packet.bytes()[..4].try_into().expect("count")),
        2
    );
    let resource = packet
        .resource("vignette_radial")
        .expect("vignette records");
    assert_eq!(resource.extent(), [2, 1, 1]);
    let first = &resource.bytes()[..28];
    assert_eq!(
        f32::from_ne_bytes(first[..4].try_into().expect("coefficient")).to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        f32::from_ne_bytes(first[20..24].try_into().expect("center x")).to_bits(),
        0.45_f32.to_bits()
    );
    assert_eq!(
        f32::from_ne_bytes(first[24..28].try_into().expect("center y")).to_bits(),
        0.55_f32.to_bits()
    );
}

#[test]
fn lsc_preprocess_rejects_values_outside_gpu_range() {
    let error = rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess(
            "00",
            &context(vec![VignetteRadialParameters {
                coefficients: [f64::MAX, 0.0, 0.0, 0.0, 0.0],
                optical_center: [0.5, 0.5],
            }]),
        )
        .expect_err("finite f64 values that overflow f32 must fail");

    assert_eq!(
        error.to_string(),
        "operator `lsc` preprocessing failed: FixVignetteRadial values must be finite, GPU-representable, and centers within [0, 1]"
    );
}

#[test]
fn lsc_preprocess_uses_identity_record_when_no_vignette_exists() {
    let packet = rime_isp::operator_by_id("lsc")
        .expect("LSC")
        .preprocess("00", &context(Vec::new()))
        .expect("LSC preprocessing");

    assert_eq!(
        u32::from_ne_bytes(packet.bytes()[..4].try_into().expect("count")),
        0
    );
    let resource = packet.resource("vignette_radial").expect("identity record");
    assert_eq!(resource.extent(), [1, 1, 1]);
    assert!(resource.bytes().iter().all(|byte| *byte == 0));
}
