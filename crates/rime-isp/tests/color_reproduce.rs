//! Color reproduce preprocessing: dual-illuminant weight iteration and
//! sensor-to-ProPhoto matrix synthesis, strictly following the MATLAB
//! reference `color_reproduce_dng.m` and its `toolbox/matlab_dng` helpers.
#![expect(
    clippy::unreadable_literal,
    reason = "golden reference values use separated digit literals"
)]

//! Color reproduce preprocessing: dual-illuminant weight iteration and
//! sensor-to-ProPhoto matrix synthesis, strictly following the MATLAB
//! reference `color_reproduce_dng.m` and its `toolbox/matlab_dng` helpers.

use rime_isp::vbe::color_reproduce::solver::{ColorReproduceInputs, solve_color_reproduce};

fn matrix(values: [f64; 9]) -> [[f64; 3]; 3] {
    [
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ]
}

/// GH5S P1020601 tag values (SRATIONAL exact).
fn gh5s_inputs() -> ColorReproduceInputs {
    ColorReproduceInputs {
        color_matrix1: matrix([
            0.7718, -0.3541, 0.0141, -0.2768, 1.0432, 0.2711, -0.0242, 0.0926, 0.6051,
        ]),
        color_matrix2: Some(matrix([
            0.6929, -0.2355, -0.0708, -0.4192, 1.2534, 0.1828, -0.1097, 0.1989, 0.5195,
        ])),
        camera_calibration1: None,
        camera_calibration2: None,
        signatures_match: false,
        analog_balance: Some([1.0, 1.0, 1.0]),
        as_shot_neutral: [0.356546, 1.0, 0.573991],
        calibration_illuminant1: 17,
        calibration_illuminant2: Some(21),
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{label}: expected {expected}, got {actual}"
    );
}

#[test]
fn gh5s_weight_iteration_converges_to_reference() {
    let solution = solve_color_reproduce(&gh5s_inputs()).expect("GH5S inputs must solve");

    // Design doc §10.1 golden values (f64 reference, tolerance 1e-6).
    assert_close(solution.weight1, 0.128_334_645_2, 1e-6, "weight1");
    assert_close(solution.weight2, 0.871_665_354_8, 1e-6, "weight2");
    assert_close(
        solution.xyz_neutral[0],
        0.330_446_644_5,
        1e-6,
        "xyzNeutral.x",
    );
    assert_close(
        solution.xyz_neutral[1],
        0.345_507_197_8,
        1e-6,
        "xyzNeutral.y",
    );
    assert_close(
        solution.xyz_neutral[2],
        0.324_046_157_7,
        1e-6,
        "xyzNeutral.z",
    );

    let camera_to_xyz_d50 = solution.camera_to_xyz_d50;
    let expected = [
        [1.689_336_940_1, 0.355_474_045_5, 0.011_149_348_7],
        [0.536_398_182_8, 0.994_140_869_7, -0.322_986_764_8],
        [0.109_323_269_5, -0.252_270_898_2, 1.808_725_439_5],
    ];
    for row in 0..3 {
        for col in 0..3 {
            assert_close(
                camera_to_xyz_d50[row][col],
                expected[row][col],
                1e-6,
                &format!("camera_to_xyz_d50[{row}][{col}]"),
            );
        }
    }

    let sensor_to_prophoto = solution.sensor_to_prophoto;
    let expected = [
        [0.759_738_240_0, 0.237_206_610_9, 0.002_936_844_4],
        [-0.038_789_541_2, 1.300_616_004_6, -0.261_778_557_5],
        [0.047_241_009_6, -0.305_744_141_4, 1.258_255_160_1],
    ];
    for row in 0..3 {
        for col in 0..3 {
            assert_close(
                sensor_to_prophoto[row][col],
                expected[row][col],
                1e-6,
                &format!("sensor_to_prophoto[{row}][{col}]"),
            );
        }
    }
}

#[test]
fn gh5s_white_point_closes_through_full_chain() {
    let solution = solve_color_reproduce(&gh5s_inputs()).expect("GH5S inputs must solve");

    // A WB'd neutral gray (t,t,t) must map to ProPhoto white (~1,1,1):
    // the folded diag(N) exactly cancels the WBC gains (white closure).
    let prophoto = mul_vec(solution.sensor_to_prophoto, [1.0, 1.0, 1.0]);
    for channel in prophoto {
        assert_close(channel, 1.0, 2e-3, "white closure");
    }

    // The same neutral through both matrices lands on sRGB white.
    let srgb = mul_vec(solution.prophoto_to_srgb, prophoto);
    for channel in srgb {
        assert_close(channel, 1.0, 2e-3, "sRGB white closure");
    }
}

#[test]
fn prophoto_to_srgb_matches_reference_constant() {
    let solution = solve_color_reproduce(&gh5s_inputs()).expect("GH5S inputs must solve");
    let expected = [
        [2.036_832_436_8, -0.737_559_050_4, -0.299_173_874_3],
        [-0.225_856_652_7, 1.223_126_204_8, 0.002_693_452_6],
        [-0.010_602_523_6, -0.134_855_939_1, 1.145_577_890_3],
    ];
    for (row_index, (actual_row, expected_row)) in solution
        .prophoto_to_srgb
        .iter()
        .zip(expected.iter())
        .enumerate()
    {
        for (col_index, (actual, expected_value)) in
            actual_row.iter().zip(expected_row.iter()).enumerate()
        {
            assert_close(
                *actual,
                *expected_value,
                1e-6,
                &format!("prophoto_to_srgb[{row_index}][{col_index}]"),
            );
        }
    }
}

#[test]
fn single_illuminant_falls_back_to_full_first_matrix_weight() {
    let inputs = ColorReproduceInputs {
        color_matrix2: None,
        calibration_illuminant2: None,
        ..gh5s_inputs()
    };
    let solution = solve_color_reproduce(&inputs).expect("single-illuminant inputs must solve");
    assert_close(solution.weight1, 1.0, 1e-12, "weight1");
    assert_close(solution.weight2, 0.0, 1e-12, "weight2");
}

#[test]
fn cold_first_illuminant_slots_are_reordered_not_rejected() {
    // GH5S matrices with DJI-style cold-first slots (CI1=D65, CI2=StdA):
    // pairs swap warm-first; both matrices keep their own illuminants.
    let inputs = ColorReproduceInputs {
        calibration_illuminant1: 21,
        calibration_illuminant2: Some(17),
        ..gh5s_inputs()
    };
    let solution =
        solve_color_reproduce(&inputs).expect("cold-first slots must solve by reordering");
    assert!(
        solution.weight1 >= 0.0 && solution.weight1 <= 1.0,
        "weights must stay in [0, 1]: {}",
        solution.weight1
    );
}

#[test]
fn unsupported_illuminant_code_is_rejected() {
    let inputs = ColorReproduceInputs {
        calibration_illuminant1: 13,
        ..gh5s_inputs()
    };
    let error = solve_color_reproduce(&inputs).expect_err("fluorescent code unsupported");
    assert!(
        error
            .to_string()
            .contains("unsupported calibration illuminant"),
        "unexpected error: {error}"
    );
}

#[test]
fn signatures_match_enables_camera_calibration_matrices() {
    // With matching signatures and non-identity CC1, the XYZtoCamera chain
    // must incorporate CC1 (matrix product differs from the identity case).
    let inputs = ColorReproduceInputs {
        camera_calibration1: Some(matrix([1.1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.9])),
        camera_calibration2: Some(matrix([1.2, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.8])),
        signatures_match: true,
        ..gh5s_inputs()
    };
    let solution = solve_color_reproduce(&inputs).expect("calibrated inputs must solve");
    let identity = solve_color_reproduce(&gh5s_inputs()).expect("identity inputs must solve");
    let differs = solution.camera_to_xyz_d50 != identity.camera_to_xyz_d50;
    assert!(differs, "camera calibration must change the chain");
}

fn mul_vec(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
