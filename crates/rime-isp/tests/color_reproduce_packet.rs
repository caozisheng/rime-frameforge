//! Packet contract for the color reproduce operator preprocess.
#![expect(
    clippy::unreadable_literal,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "golden reference values use f32-narrowed synthetic tables"
)]

use rime_isp::vbe::color_reproduce::solver::{
    ColorReproduceInputs, ColorReproduceSolution, solve_color_reproduce,
};

use rime_isp::{FrameIdentity, PreprocessContext};

fn matrix(values: [f64; 9]) -> [[f64; 3]; 3] {
    [
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ]
}

fn gh5s_context() -> PreprocessContext {
    let mut context = minimal_context();
    context.color_matrix1 = [
        0.7718, -0.3541, 0.0141, -0.2768, 1.0432, 0.2711, -0.0242, 0.0926, 0.6051,
    ];
    context.color_matrix2 = Some([
        0.6929, -0.2355, -0.0708, -0.4192, 1.2534, 0.1828, -0.1097, 0.1989, 0.5195,
    ]);
    context.calibration_illuminant1_code = Some(17);
    context.calibration_illuminant2_code = Some(21);
    context.as_shot_neutral = Some([0.356546, 1.0, 0.573991]);
    context.profile_calibration_signature = Some("com.adobe".into());
    context.profile_hue_sat_map_dims = Some([2, 3, 1]);
    context.profile_hue_sat_map_data1 = Some((0..3 * 2 * 3).map(|i| i as f32 * 0.01).collect());
    context.profile_hue_sat_map_data2 =
        Some((0..3 * 2 * 3).map(|i| 1.0 + i as f32 * 0.02).collect());
    context
}

fn minimal_context() -> PreprocessContext {
    PreprocessContext {
        identity: FrameIdentity {
            frame_index: 1,
            run_revision: 1,
            method_revision: 1,
        },
        width: 1,
        height: 1,
        black_level: 0.0,
        white_level: 1.0,
        cfa_pattern: [0, 1, 1, 2],
        as_shot_neutral: Some([1.0, 1.0, 1.0]),
        as_shot_white_xy: None,
        color_matrix1: [1.0; 9],
        color_matrix2: None,
        calibration_illuminant1_code: Some(21),
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
        wbc_highlight_recovery: false,
    }
}

fn read_f32(bytes: &[u8], index: usize) -> f32 {
    f32::from_ne_bytes(
        bytes[index * 4..index * 4 + 4]
            .try_into()
            .expect("f32 slice"),
    )
}

fn read_u32(bytes: &[u8], index: usize) -> u32 {
    u32::from_ne_bytes(
        bytes[index * 4..index * 4 + 4]
            .try_into()
            .expect("u32 slice"),
    )
}

#[test]
fn packet_freezes_matrices_lut_and_uniform() {
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR operator");
    let packet = operator
        .preprocess("00", &gh5s_context())
        .expect("CR preprocess");

    // Uniform: 16 bytes, hue/saturation dims + hs enable (z) + reserved (w).
    assert_eq!(packet.bytes().len(), 16);
    assert_eq!(read_u32(packet.bytes(), 0), 2);
    assert_eq!(read_u32(packet.bytes(), 1), 3);
    assert_eq!(read_u32(packet.bytes(), 2), 1);
    assert_eq!(read_u32(packet.bytes(), 3), 0);

    // Matrices resource: 18 f32 row-major, sensor->ProPhoto then ProPhoto->sRGB.
    let matrices = packet.resource("cr_matrices").expect("cr_matrices");
    assert_eq!(matrices.bytes().len(), 18 * 4);
    let solution: ColorReproduceSolution =
        solve_color_reproduce(&inputs_of(&gh5s_context())).expect("solver agrees");
    for row in 0..3 {
        for col in 0..3 {
            let expected = solution.sensor_to_prophoto[row][col] as f32;
            let actual = read_f32(matrices.bytes(), row * 3 + col);
            assert!(
                (expected - actual).abs() < 1e-5,
                "sensor_to_prophoto[{row}][{col}]: {expected} vs {actual}"
            );
            let expected = solution.prophoto_to_srgb[row][col] as f32;
            let actual = read_f32(matrices.bytes(), 9 + row * 3 + col);
            assert!(
                (expected - actual).abs() < 1e-5,
                "prophoto_to_srgb[{row}][{col}]: {expected} vs {actual}"
            );
        }
    }

    // HS LUT resource: per-element illuminant interpolation of the two tables.
    let lut = packet.resource("cr_hs_lut").expect("cr_hs_lut");
    assert_eq!(lut.bytes().len(), 3 * 2 * 3 * 4);
    let weight1 = solution.weight1 as f32;
    let weight2 = solution.weight2 as f32;
    for index in 0..3 * 2 * 3 {
        let expected = weight1 * gh5s_context().profile_hue_sat_map_data1.as_ref().unwrap()[index]
            + weight2 * gh5s_context().profile_hue_sat_map_data2.as_ref().unwrap()[index];
        let actual = read_f32(lut.bytes(), index);
        assert!(
            (expected - actual).abs() < 1e-6,
            "lut[{index}]: {expected} vs {actual}"
        );
    }
}

#[test]
fn missing_hs_dims_disables_lut_but_keeps_matrices() {
    let mut context = gh5s_context();
    context.profile_hue_sat_map_dims = None;
    context.profile_hue_sat_map_data1 = None;
    context.profile_hue_sat_map_data2 = None;
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR operator");
    let packet = operator.preprocess("00", &context).expect("CR preprocess");
    assert_eq!(read_u32(packet.bytes(), 2), 0);
    assert!(packet.resource("cr_matrices").is_some());
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}

#[test]
fn missing_illuminant_code_is_rejected() {
    let mut context = gh5s_context();
    context.calibration_illuminant1_code = None;
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR operator");
    let error = operator
        .preprocess("00", &context)
        .expect_err("illuminant required");
    assert!(
        error.to_string().contains("missing calibration illuminant"),
        "unexpected error: {error}"
    );
}

#[test]
fn missing_as_shot_neutral_falls_back_to_white_xy() {
    // No AsShotNeutral: the wbc00-style fallback from AsShotWhiteXY must run.
    let mut context = gh5s_context();
    context.as_shot_neutral = None;
    context.as_shot_white_xy = Some([0.3127, 0.3290]);
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR operator");
    let packet = operator
        .preprocess("00", &context)
        .expect("white xy fallback solves");
    assert!(packet.resource("cr_matrices").is_some());
}

fn inputs_of(context: &PreprocessContext) -> ColorReproduceInputs {
    ColorReproduceInputs {
        color_matrix1: matrix(context.color_matrix1),
        color_matrix2: context.color_matrix2.map(matrix),
        camera_calibration1: context.camera_calibration1.map(matrix),
        camera_calibration2: context.camera_calibration2.map(matrix),
        signatures_match: context
            .camera_calibration_signature
            .as_ref()
            .is_some_and(|camera| Some(camera) == context.profile_calibration_signature.as_ref()),
        analog_balance: context.analog_balance,
        as_shot_neutral: context.as_shot_neutral.expect("neutral present"),
        calibration_illuminant1: context.calibration_illuminant1_code.expect("ci1"),
        calibration_illuminant2: context.calibration_illuminant2_code,
    }
}

#[test]
fn solver_agrees_with_packet_matrices() {
    let solution = solve_color_reproduce(&inputs_of(&gh5s_context())).expect("solve");
    assert!((solution.weight1 - 0.128_334_645_2).abs() < 1e-4);
}
