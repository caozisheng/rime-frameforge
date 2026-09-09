//! Native GPU consistency for the color reproduce operator on the GH5S
//! fixture: the full graph (including CR matrices + HSV LUT storage
//! bindings) must read back finite pixels, and the CR packet must match the
//! CPU solver's converged weights.

use std::path::Path;

use rime_dng::DngReader;
use rime_isp::vbe::color_reproduce::solver::{ColorReproduceInputs, solve_color_reproduce};
use rime_native_gpu::{NativeFrameIdentity, WgpuReadbackError, WgpuReadbackExecutor};

const GH5S_SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../pipeline/normal/P1020601.dng"
);

#[test]
fn gh5s_full_graph_with_color_reproduce_reads_back_finite() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 21)
        .expect("GH5S DNG must decode");
    let executor = match WgpuReadbackExecutor::new() {
        Ok(executor) => executor,
        Err(WgpuReadbackError::AdapterUnavailable) => return,
        Err(error) => panic!("native GPU must initialize when an adapter exists: {error}"),
    };

    // The graph dispatches CR with its uniform + two storage bindings.
    let surface = executor
        .render_with_identity(
            &frame,
            NativeFrameIdentity {
                frame_index: 21,
                run_revision: 1,
                method_revision: 1,
                gpu_generation: 1,
                phase: rime_core::FramePhase::Output,
            },
        )
        .expect("render with color reproduce must read back");
    assert!(
        surface.pixels().iter().all(|value| value.is_finite()),
        "CR output must stay finite"
    );
}

#[test]
fn gh5s_solver_and_packet_agree_on_converged_weights() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 22)
        .expect("GH5S DNG must decode");
    let metadata = &frame.metadata;
    let matrix = |flat: [f64; 9]| {
        [
            [flat[0], flat[1], flat[2]],
            [flat[3], flat[4], flat[5]],
            [flat[6], flat[7], flat[8]],
        ]
    };
    let solution = solve_color_reproduce(&ColorReproduceInputs {
        color_matrix1: matrix(metadata.color_matrix1),
        color_matrix2: metadata.color_matrix2.map(matrix),
        camera_calibration1: metadata.camera_calibration1.map(matrix),
        camera_calibration2: metadata.camera_calibration2.map(matrix),
        signatures_match: match (
            &metadata.camera_calibration_signature,
            &metadata.profile_calibration_signature,
        ) {
            (Some(camera), Some(profile)) => camera == profile,
            _ => false,
        },
        analog_balance: metadata.analog_balance,
        as_shot_neutral: metadata.as_shot_neutral.expect("GH5S neutral"),
        calibration_illuminant1: metadata.calibration_illuminant1_code.expect("CI1"),
        calibration_illuminant2: metadata.calibration_illuminant2_code,
    })
    .expect("solver must reproduce the same matrices");
    // White closure: the GH5S golden weight from the design document.
    assert!(
        (solution.weight1 - 0.128_334_645_2).abs() < 1e-3,
        "solver weight drifted: {}",
        solution.weight1
    );

    // The packet's HSV LUT must equal the same weighted interpolation.
    let operator = rime_isp::operator_by_id("color_reproduce").expect("CR");
    let mut context = gpu_suite_context();
    context.color_matrix1 = metadata.color_matrix1;
    context.color_matrix2 = metadata.color_matrix2;
    context.calibration_illuminant1_code = metadata.calibration_illuminant1_code;
    context.calibration_illuminant2_code = metadata.calibration_illuminant2_code;
    context.as_shot_neutral = metadata.as_shot_neutral;
    context.profile_hue_sat_map_dims = metadata.profile_hue_sat_map_dims;
    context.profile_hue_sat_map_data1 = metadata.profile_hue_sat_map_data1.clone();
    context.profile_hue_sat_map_data2 = metadata.profile_hue_sat_map_data2.clone();
    let packet = operator.preprocess("00", &context).expect("CR packet");

    let lut = packet.resource("cr_hsv_lut").expect("interpolated LUT");
    let bytes = lut.bytes();
    let dims = metadata.profile_hue_sat_map_dims.expect("dims");
    assert_eq!(
        bytes.len(),
        3 * dims[0] as usize * dims[1] as usize * dims[2] as usize * 4
    );
    let read = |index: usize| {
        f32::from_ne_bytes(bytes[index * 4..index * 4 + 4].try_into().expect("slice"))
    };
    let data1 = metadata.profile_hue_sat_map_data1.as_ref().unwrap();
    let data2 = metadata.profile_hue_sat_map_data2.as_ref().unwrap();
    for index in [0_usize, 4049, 8099] {
        let actual = read(index);
        let expected =
            solution.weight1 * f64::from(data1[index]) + solution.weight2 * f64::from(data2[index]);
        assert!(
            (f64::from(actual) - expected).abs() < 1e-4,
            "lut[{index}]: {actual} vs {expected}"
        );
    }
}

fn gpu_suite_context() -> rime_isp::PreprocessContext {
    rime_isp::PreprocessContext {
        identity: rime_isp::FrameIdentity {
            frame_index: 22,
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
