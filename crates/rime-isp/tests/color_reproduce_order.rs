//! Color reproduce preprocessing: dual-illuminant weight iteration and
//! sensor-to-ProPhoto matrix synthesis, strictly following the MATLAB
//! reference `color_reproduce_dng.m` and its `toolbox/matlab_dng` helpers.

#![expect(
    clippy::unreadable_literal,
    reason = "golden reference values use separated digit literals"
)]

use rime_isp::vbe::color_reproduce::solver::{ColorReproduceInputs, solve_color_reproduce};

fn matrix(values: [f64; 9]) -> [[f64; 3]; 3] {
    [
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ]
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{label}: expected {expected}, got {actual}"
    );
}

/// DJI X5S fixture tag values: the illuminant slots are cold-first
/// (50778=D65, 50779=StdA) with no HSV calibration. The solver must pair
/// each matrix with its illuminant and reorder warm-first instead of
/// rejecting the frame.
fn x5s_inputs() -> ColorReproduceInputs {
    ColorReproduceInputs {
        color_matrix1: matrix([
            0.9665, -0.3201, -0.1186, -0.3631, 1.0887, 0.1188, -0.0969, 0.2666, 0.5813,
        ]),
        color_matrix2: Some(matrix([
            1.4584, -0.2110, -0.0791, -1.0170, 0.8483, 0.0787, 0.0543, 0.4305, 0.5208,
        ])),
        camera_calibration1: None,
        camera_calibration2: None,
        signatures_match: false,
        analog_balance: Some([1.0, 1.0, 1.0]),
        as_shot_neutral: [0.629476, 1.0, 0.455567],
        calibration_illuminant1: 21,
        calibration_illuminant2: Some(17),
    }
}

#[test]
fn cold_first_illuminant_order_is_reordered_not_rejected() {
    let solution = solve_color_reproduce(&x5s_inputs()).expect("X5S inputs must solve");

    // Python golden for the warm-first reorder (fresh D50 start; the fixed
    // point is start-independent): w1 pairs CM1 with D65 (cold weight).
    assert_close(solution.weight1, 0.715_863_898_4, 1e-5, "weight1");
    assert_close(solution.weight2, 0.284_136_101_6, 1e-5, "weight2");
    assert_close(
        solution.xyz_neutral[0],
        0.381_838_728_3,
        1e-5,
        "xyzNeutral.x",
    );
    assert_close(
        solution.xyz_neutral[1],
        0.589_848_140_7,
        1e-5,
        "xyzNeutral.y",
    );

    // White closure still holds through the folded matrix.
    let prophoto = mul_vec(solution.sensor_to_prophoto, [1.0, 1.0, 1.0]);
    for channel in prophoto {
        assert_close(channel, 1.0, 2e-3, "white closure");
    }
}

#[test]
fn identical_illuminants_do_not_divide_by_zero() {
    let inputs = ColorReproduceInputs {
        calibration_illuminant2: Some(21),
        color_matrix2: Some(matrix([
            0.6929, -0.2355, -0.0708, -0.4192, 1.2534, 0.1828, -0.1097, 0.1989, 0.5195,
        ])),
        ..x5s_inputs()
    };
    let solution = solve_color_reproduce(&inputs).expect("degenerate dual must solve");
    // Both slots are D65: the weight degenerates; the first matrix takes over.
    assert_close(solution.weight1, 1.0, 1e-12, "weight1");
}

fn mul_vec(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
