use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WhiteBalanceMetadata {
    pub as_shot_neutral: Option<[f64; 3]>,
    pub as_shot_white_xy: Option<[f64; 2]>,
    pub color_matrix1: [f64; 9],
    pub color_matrix2: Option<[f64; 9]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WhiteBalanceGains {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum WhiteBalanceError {
    #[error("missing AsShotNeutral and AsShotWhiteXY")]
    MissingSource,
    #[error("invalid AsShotNeutral components")]
    InvalidNeutral,
    #[error("invalid AsShotWhiteXY chromaticity")]
    InvalidWhiteXy,
    #[error("invalid color matrix")]
    InvalidColorMatrix,
    #[error("white point maps to invalid camera values")]
    InvalidCameraWhite,
    #[error("white balance gains are not finite and positive")]
    InvalidGains,
}

/// Resolves DNG as-shot metadata into green-normalized RGB gains for WBC.
///
/// # Errors
///
/// Returns a stable validation error when the source metadata cannot produce
/// finite, strictly positive camera-channel gains.
pub fn white_balance_gains(
    metadata: &WhiteBalanceMetadata,
) -> Result<WhiteBalanceGains, WhiteBalanceError> {
    let neutral = match metadata.as_shot_neutral {
        Some(neutral) => validate_neutral(neutral)?,
        None => neutral_from_white_xy(
            metadata
                .as_shot_white_xy
                .ok_or(WhiteBalanceError::MissingSource)?,
            metadata.color_matrix2.unwrap_or(metadata.color_matrix1),
        )?,
    };
    let gains = [1.0 / neutral[0], 1.0 / neutral[1], 1.0 / neutral[2]];
    let normalized = [gains[0] / gains[1], 1.0, gains[2] / gains[1]];
    if !normalized
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(WhiteBalanceError::InvalidGains);
    }
    let gains = WhiteBalanceGains {
        red: narrow_gain(normalized[0]),
        green: narrow_gain(normalized[1]),
        blue: narrow_gain(normalized[2]),
    };
    if ![gains.red, gains.green, gains.blue]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(WhiteBalanceError::InvalidGains);
    }
    Ok(gains)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "validated finite f64 gains are narrowed to the GPU f32 parameter contract"
)]
fn narrow_gain(value: f64) -> f32 {
    value as f32
}

fn validate_neutral(neutral: [f64; 3]) -> Result<[f64; 3], WhiteBalanceError> {
    neutral
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
        .then_some(neutral)
        .ok_or(WhiteBalanceError::InvalidNeutral)
}

fn neutral_from_white_xy(
    white_xy: [f64; 2],
    color_matrix: [f64; 9],
) -> Result<[f64; 3], WhiteBalanceError> {
    let [x, y] = white_xy;
    if !x.is_finite() || !y.is_finite() || x <= 0.0 || y <= 0.0 || x + y >= 1.0 {
        return Err(WhiteBalanceError::InvalidWhiteXy);
    }
    if !color_matrix.iter().all(|value| value.is_finite()) {
        return Err(WhiteBalanceError::InvalidColorMatrix);
    }
    let xyz = [x / y, 1.0, (1.0 - x - y) / y];
    let camera = [
        color_matrix[0] * xyz[0] + color_matrix[1] * xyz[1] + color_matrix[2] * xyz[2],
        color_matrix[3] * xyz[0] + color_matrix[4] * xyz[1] + color_matrix[5] * xyz[2],
        color_matrix[6] * xyz[0] + color_matrix[7] * xyz[1] + color_matrix[8] * xyz[2],
    ];
    if !camera.iter().all(|value| value.is_finite() && *value > 0.0) {
        return Err(WhiteBalanceError::InvalidCameraWhite);
    }
    validate_neutral([camera[0] / camera[1], 1.0, camera[2] / camera[1]])
}
