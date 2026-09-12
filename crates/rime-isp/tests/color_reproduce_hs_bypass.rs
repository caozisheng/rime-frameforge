//! HS calibration bypass semantics: a frame without a usable (complete,
//! size-consistent, `ValueDivs == 1`) HS LUT must still render through the
//! matrix chain with the lookup disabled, never fail the whole color
//! reproduce preprocess.

#![expect(
    clippy::unreadable_literal,
    reason = "golden reference values use unsuffixed compact literals"
)]

use rime_isp::{FrameIdentity, PreprocessContext};

fn gh5s_context() -> PreprocessContext {
    let mut context = base_context();
    context.color_matrix1 = [
        0.7718, -0.3541, 0.0141, -0.2768, 1.0432, 0.2711, -0.0242, 0.0926, 0.6051,
    ];
    context.color_matrix2 = Some([
        0.6929, -0.2355, -0.0708, -0.4192, 1.2534, 0.1828, -0.1097, 0.1989, 0.5195,
    ]);
    context.calibration_illuminant1_code = Some(17);
    context.calibration_illuminant2_code = Some(21);
    context.as_shot_neutral = Some([0.356546, 1.0, 0.573991]);
    context
}

fn base_context() -> PreprocessContext {
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
        drc_modulation_curves: None,
        wbc_highlight_recovery: false,
        wbc_hr_gain: None,
        drc_details_amplify: true,
    }
}

fn read_u32(bytes: &[u8], index: usize) -> u32 {
    u32::from_ne_bytes(
        bytes[index * 4..index * 4 + 4]
            .try_into()
            .expect("u32 slice"),
    )
}

fn cr_packet(context: &PreprocessContext) -> rime_isp::ModuleParameterPacket {
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR operator");
    operator.preprocess("00", context).expect("CR preprocess")
}

#[test]
fn dji_x5s_frame_solves_without_hs_calibration() {
    // X5S slots are cold-first and the file carries no HSV map tags at all.
    let mut context = base_context();
    context.color_matrix1 = [
        0.9665, -0.3201, -0.1186, -0.3631, 1.0887, 0.1188, -0.0969, 0.2666, 0.5813,
    ];
    context.color_matrix2 = Some([
        1.4584, -0.2110, -0.0791, -1.0170, 0.8483, 0.0787, 0.0543, 0.4305, 0.5208,
    ]);
    context.calibration_illuminant1_code = Some(21);
    context.calibration_illuminant2_code = Some(17);
    context.as_shot_neutral = Some([0.629476, 1.0, 0.455567]);

    let packet = cr_packet(&context);
    assert_eq!(read_u32(packet.bytes(), 2), 0, "HS must be bypassed");
    assert!(
        packet.resource("cr_matrices").is_some(),
        "matrices must still ship"
    );
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}

#[test]
fn dims_without_any_table_bypasses_hs_instead_of_failing() {
    let mut context = gh5s_context();
    context.profile_hue_sat_map_dims = Some([90, 30, 1]);
    context.profile_hue_sat_map_data1 = None;
    context.profile_hue_sat_map_data2 = None;

    let packet = cr_packet(&context);
    assert_eq!(read_u32(packet.bytes(), 2), 0, "HS must be bypassed");
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}

#[test]
fn dims_with_mismatched_table_sizes_bypasses_hs_instead_of_failing() {
    let mut context = gh5s_context();
    context.profile_hue_sat_map_dims = Some([90, 30, 1]);
    context.profile_hue_sat_map_data1 = Some(vec![0.0; 8100]);
    context.profile_hue_sat_map_data2 = Some(vec![0.0; 100]);

    let packet = cr_packet(&context);
    assert_eq!(read_u32(packet.bytes(), 2), 0, "HS must be bypassed");
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}

#[test]
fn dual_illuminant_with_a_single_table_bypasses_hs_instead_of_failing() {
    let mut context = gh5s_context();
    context.profile_hue_sat_map_dims = Some([90, 30, 1]);
    context.profile_hue_sat_map_data1 = Some(vec![1.0; 8100]);
    context.profile_hue_sat_map_data2 = None;

    let packet = cr_packet(&context);
    assert_eq!(read_u32(packet.bytes(), 2), 0, "HS must be bypassed");
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}

#[test]
fn value_divs_above_one_bypasses_hs_instead_of_slicing_layers() {
    // DNG permits ValueDivs > 1 but industrial profiles never use it;
    // approximating with the first value layer is forbidden — bypass.
    let mut context = gh5s_context();
    context.profile_hue_sat_map_dims = Some([90, 30, 2]);
    context.profile_hue_sat_map_data1 = Some(vec![1.0; 3 * 90 * 30 * 2]);
    context.profile_hue_sat_map_data2 = Some(vec![1.0; 3 * 90 * 30 * 2]);

    let packet = cr_packet(&context);
    assert_eq!(read_u32(packet.bytes(), 2), 0, "HS must be bypassed");
    assert!(
        packet.resource("cr_matrices").is_some(),
        "matrices must still ship"
    );
    assert!(
        packet.resource("cr_hs_lut").is_some(),
        "placeholder resource must ship"
    );
}
