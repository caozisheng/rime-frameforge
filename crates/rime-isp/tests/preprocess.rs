use rime_isp::preprocess::{WhiteBalanceMetadata, white_balance_gains};

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
    })
    .expect("white xy is valid");

    assert!((gains.red - 0.5).abs() < 1e-6);
    assert!((gains.green - 1.0).abs() < 1e-6);
    assert!((gains.blue - 2.0).abs() < 1e-6);
}

#[test]
fn invalid_white_balance_metadata_is_rejected() {
    let error = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: None,
        as_shot_white_xy: Some([0.0, 0.25]),
        color_matrix1: matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        color_matrix2: None,
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
    })
    .expect_err("GPU f32 overflow must fail");

    assert_eq!(
        error.to_string(),
        "white balance gains are not finite and positive"
    );
}
