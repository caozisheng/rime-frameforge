use rime_isp::{FrameIdentity, PreprocessContext};
use rime_native_gpu::{OperatorPhase, execute_operator_phases};

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
