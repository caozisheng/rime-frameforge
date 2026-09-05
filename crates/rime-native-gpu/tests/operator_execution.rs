use rime_isp::{FrameIdentity, PreprocessContext};
use rime_native_gpu::{execute_operator_phases, OperatorPhase};

#[test]
fn scheduler_runs_all_cpu_preprocess_before_compute_and_postprocess() {
    let context = PreprocessContext {
        identity: FrameIdentity {
            frame_index: 7,
            run_revision: 1,
            method_revision: 2,
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
        scene_brightness_ev: None,
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
    };
    let events = execute_operator_phases(&["blc", "wbc"], &context, |_operator, _packet| Ok(()))
        .expect("operator phases must succeed");

    let first_compute = events
        .iter()
        .position(|event| event.phase == OperatorPhase::Compute)
        .expect("compute event");
    let last_preprocess = events
        .iter()
        .rposition(|event| event.phase == OperatorPhase::Preprocess)
        .expect("preprocess event");
    let first_postprocess = events
        .iter()
        .position(|event| event.phase == OperatorPhase::Postprocess)
        .expect("postprocess event");
    assert!(last_preprocess < first_compute);
    assert!(first_compute < first_postprocess);
}

#[test]
fn scheduler_uses_the_selected_method_for_all_three_phases() {
    let context = PreprocessContext {
        identity: FrameIdentity {
            frame_index: 9,
            run_revision: 1,
            method_revision: 3,
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
        scene_brightness_ev: Some(8.0),
        exposure_deviation_ev: None,
        iso: Some(100.0),
        analog_gain: None,
        digital_gain: None,
    };
    let events = rime_native_gpu::execute_operator_methods(
        &[("dem", "04")],
        &context,
        |_operator, packet| {
            assert_eq!(packet.method(), "04");
            Ok(())
        },
    )
    .expect("selected DEM method must execute");

    assert!(events.iter().all(|event| event.method == "04"));
}

#[test]
fn ahd_preprocess_accepts_scene_brightness_without_iso() {
    let context = PreprocessContext {
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
        scene_brightness_ev: Some(4.0),
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
    };
    let result = rime_isp::operator_by_id("dem")
        .expect("DEM")
        .preprocess("04", &context);
    assert!(result.is_ok());
}

#[test]
fn gamma_preprocess_emits_default_gamma_and_identity_luminance_lut() {
    let context = PreprocessContext {
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
        scene_brightness_ev: Some(4.0),
        exposure_deviation_ev: None,
        iso: None,
        analog_gain: None,
        digital_gain: None,
    };
    let packet = rime_isp::operator_by_id("gamma")
        .expect("Gamma")
        .preprocess("00", &context)
        .expect("Gamma preprocess");
    assert_eq!(packet.bytes().len(), 64);
    let gamma = f32::from_ne_bytes(packet.bytes()[0..4].try_into().expect("gamma bytes"));
    assert!((gamma - 2.2).abs() < f32::EPSILON);
    let values = (0..9)
        .map(|index| {
            f32::from_ne_bytes(
                packet.bytes()[16 + index * 4..20 + index * 4]
                    .try_into()
                    .expect("LUT bytes"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0]
    );
}
