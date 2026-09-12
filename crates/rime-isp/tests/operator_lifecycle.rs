use rime_isp::{
    FrameIdentity, OperatorPhase, PreprocessContext, complete_operator_methods,
    prepare_operator_methods,
};

fn context() -> PreprocessContext {
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
        as_shot_neutral: Some([0.25, 1.0, 0.5]),
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
        wbc_highlight_recovery: true,
        wbc_hr_gain: None,
        drc_details_amplify: true,
    }
}

#[test]
fn shared_lifecycle_splits_preprocess_and_postprocess_around_compute() {
    let prepared =
        prepare_operator_methods(&[("blc", "00"), ("wbc", "00"), ("drc", "00")], &context())
            .expect("shared preprocess");

    let module_ids = prepared
        .packets()
        .iter()
        .map(rime_isp::ModuleParameterPacket::module_id)
        .collect::<Vec<_>>();
    assert_eq!(module_ids, ["blc", "wbc", "drc"]);

    let drc_gain = f32::from_ne_bytes(
        prepared.packets()[2].bytes()[0..4]
            .try_into()
            .expect("DRC gain bytes"),
    );
    assert!((drc_gain - 2.0).abs() < f32::EPSILON);
    assert_eq!(
        prepared
            .events()
            .iter()
            .map(|event| event.phase)
            .collect::<Vec<_>>(),
        [
            OperatorPhase::Preprocess,
            OperatorPhase::Preprocess,
            OperatorPhase::Preprocess,
        ]
    );

    let postprocess_events = complete_operator_methods(&prepared).expect("shared postprocess");
    assert_eq!(
        postprocess_events
            .iter()
            .map(|event| event.phase)
            .collect::<Vec<_>>(),
        [
            OperatorPhase::Postprocess,
            OperatorPhase::Postprocess,
            OperatorPhase::Postprocess,
        ]
    );
}
