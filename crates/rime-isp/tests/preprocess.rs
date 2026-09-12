use rime_isp::vfe::white_balance::{
    WhiteBalanceGains, WhiteBalanceMetadata, highlight_recovery_gain, white_balance_gains,
};

fn matrix(values: [f64; 9]) -> [f64; 9] {
    values
}

#[test]
fn explicit_as_shot_neutral_takes_precedence_over_white_xy() {
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: Some([0.5, 1.0, 0.25]),
        as_shot_white_xy: Some([0.3127, 0.3290]),
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: Some(matrix([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0])),
        analog_balance: None,
    })
    .expect("explicit neutral is valid");

    assert!((gains.red - 2.0).abs() < 1e-6);
    assert!((gains.green - 1.0).abs() < 1e-6);
    assert!((gains.blue - 4.0).abs() < 1e-6);
}

#[test]
fn white_xy_uses_color_matrix2_and_normalizes_gains_by_green() {
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.25, 0.25]),
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: Some(matrix([2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.25])),
        analog_balance: None,
    })
    .expect("white xy is valid");

    assert!((gains.red - 0.5).abs() < 1e-6);
    assert!((gains.green - 1.0).abs() < 1e-6);
    assert!((gains.blue - 2.0).abs() < 1e-6);
}

#[test]
fn white_xy_falls_back_to_color_matrix1() {
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.25, 0.25]),
        color_matrix1: matrix([2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.25]),
        color_matrix2: None,
        analog_balance: None,
    })
    .expect("white xy is valid");

    assert!((gains.red - 0.5).abs() < 1e-6);
    assert!((gains.green - 1.0).abs() < 1e-6);
    assert!((gains.blue - 2.0).abs() < 1e-6);
}

#[test]
fn white_xy_applies_dng_analog_balance_before_gain_normalization() {
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.25, 0.25]),
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: None,
        analog_balance: Some([2.0, 1.0, 0.5]),
    })
    .expect("white xy and AnalogBalance are valid");
    assert!((gains.red - 0.5).abs() < 1e-6);
    assert!((gains.green - 1.0).abs() < 1e-6);
    assert!((gains.blue - 1.0).abs() < 1e-6);
}

#[test]
fn invalid_white_balance_metadata_is_rejected() {
    let error = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.0, 0.25]),
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: None,
        analog_balance: None,
    })
    .expect_err("invalid chromaticity must fail");

    assert_eq!(error.to_string(), "invalid AsShotWhiteXY chromaticity");
}

#[test]
fn non_finite_color_matrix_is_rejected() {
    let error = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.25, 0.25]),
        color_matrix1: matrix([f64::NAN, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: None,
        analog_balance: None,
    })
    .expect_err("non-finite matrix must fail");

    assert_eq!(error.to_string(), "invalid color matrix");
}

#[test]
fn gains_that_overflow_f32_are_rejected() {
    let error = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: Some([1e-300, 1.0, 1.0]),
        as_shot_white_xy: None,
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: None,
        analog_balance: None,
    })
    .expect_err("GPU f32 overflow must fail");

    assert_eq!(
        error.to_string(),
        "white balance gains are not finite and positive"
    );
}

#[test]
fn hr_gain_matches_matlab_median_for_regular_white_balance() {
    for (red, green, blue, expected) in [
        (2.0, 1.0, 4.0, 2.0),
        (0.5, 1.0, 2.0, 1.0),
        (1.7, 1.0, 1.3, 1.3),
    ] {
        let gain = highlight_recovery_gain(&WhiteBalanceGains { red, green, blue })
            .expect("finite gains derive hr_gain");
        assert!(
            (gain - expected).abs() < 1e-6,
            "gains ({red}, {green}, {blue}): got {gain}, expected {expected}"
        );
    }
}

#[test]
fn hr_gain_clamps_to_one_when_median_is_below_one() {
    let gain = highlight_recovery_gain(&WhiteBalanceGains {
        red: 0.6,
        green: 1.0,
        blue: 0.8,
    })
    .expect("valid gains");
    assert!((gain - 1.0).abs() < f32::EPSILON);
}

#[test]
fn hr_gain_caps_container_at_four_x_when_median_is_tiny() {
    let gain = highlight_recovery_gain(&WhiteBalanceGains {
        red: 1.0,
        green: 1.0,
        blue: 9.0,
    })
    .expect("valid gains");
    assert!((gain - 9.0 / 4.0).abs() < 1e-6);
}

#[test]
fn hr_gain_rejects_non_finite_gains() {
    let error = highlight_recovery_gain(&WhiteBalanceGains {
        red: f32::NAN,
        green: 1.0,
        blue: 2.0,
    })
    .expect_err("NaN gain must fail");
    assert_eq!(
        error.to_string(),
        "white balance gains are not finite and positive"
    );
}
