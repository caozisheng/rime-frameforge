use thiserror::Error;

use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WhiteBalanceMetadata {
    pub as_shot_neutral: Option<[f64; 3]>,
    pub as_shot_white_xy: Option<[f64; 2]>,
    pub color_matrix1: [f64; 9],
    pub color_matrix2: Option<[f64; 9]>,
    pub analog_balance: Option<[f64; 3]>,
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
            metadata.analog_balance,
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

/// Derives the camera neutral from `AsShotWhiteXY` when `AsShotNeutral` is
/// absent. Shared by WBC gains and the color-reproduce matrix solver so both
/// consume the identical neutral (white-closure invariant).
///
/// # Errors
///
/// Returns a stable validation error when neither source is present or the
/// derived neutral is not finite and strictly positive.
pub fn neutral_from_metadata(
    as_shot_neutral: Option<[f64; 3]>,
    as_shot_white_xy: Option<[f64; 2]>,
    color_matrix: [f64; 9],
    analog_balance: Option<[f64; 3]>,
) -> Result<[f64; 3], WhiteBalanceError> {
    match as_shot_neutral {
        Some(neutral) => validate_neutral(neutral),
        None => neutral_from_white_xy(
            as_shot_white_xy.ok_or(WhiteBalanceError::MissingSource)?,
            color_matrix,
            analog_balance,
        ),
    }
}

/// Derives the highlight-recovery container gain (design §3.1) from
/// green-normalized gains.
///
/// # Errors
///
/// Returns a stable error when any gain is not finite and positive.
pub fn highlight_recovery_gain(gains: &WhiteBalanceGains) -> Result<f32, WhiteBalanceError> {
    if ![gains.red, gains.green, gains.blue]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(WhiteBalanceError::InvalidGains);
    }
    let sorted = {
        let mut values = [gains.red, gains.green, gains.blue];
        values.sort_by(f32::total_cmp);
        values
    };
    let median = sorted[1];
    let hr_gain = median.max(1.0).max(sorted[2] / 4.0);
    if !hr_gain.is_finite() || hr_gain <= 0.0 {
        return Err(WhiteBalanceError::InvalidGains);
    }
    Ok(hr_gain)
}

/// Decodes the highlight-recovery gain a WBC packet published at uniform
/// offset 12 (design §3.6) so downstream preprocesses can consume the exact
/// same value without recomputing it.
///
/// # Errors
///
/// Returns a stable error when the packet layout does not carry the field or
/// the decoded gain is not finite and positive.
///
/// # Panics
///
/// Never: the slice length is validated before the conversion.
pub fn hr_gain_from_packet(packet: &ModuleParameterPacket) -> Result<f32, WhiteBalanceError> {
    let bytes = packet.bytes();
    if bytes.len() < 16 {
        return Err(WhiteBalanceError::InvalidGains);
    }
    let raw = bytes[12..16].try_into().expect("validated 4-byte slice");
    let value = f32::from_ne_bytes(raw);
    if !value.is_finite() || value <= 0.0 {
        return Err(WhiteBalanceError::InvalidGains);
    }
    Ok(value)
}

pub(crate) fn preprocess(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let gains = white_balance_gains(&WhiteBalanceMetadata {
        as_shot_neutral: context.as_shot_neutral,
        as_shot_white_xy: context.as_shot_white_xy,
        color_matrix1: context.color_matrix1,
        color_matrix2: context.color_matrix2,
        analog_balance: context.analog_balance,
    })
    .map_err(|error| OperatorError::Preprocess {
        module_id,
        reason: error.reason(),
    })?;
    let mut uniform = [0_u8; 48];
    uniform[0..4].copy_from_slice(&gains.red.to_ne_bytes());
    uniform[4..8].copy_from_slice(&gains.green.to_ne_bytes());
    uniform[8..12].copy_from_slice(&gains.blue.to_ne_bytes());
    // Single point of computation (design §3.6): every consumer, GPU shader
    // or DRC preprocess, decodes this exact field instead of rederiving.
    let hr_gain = if context.wbc_highlight_recovery {
        highlight_recovery_gain(&gains).map_err(|error| OperatorError::Preprocess {
            module_id,
            reason: error.reason(),
        })?
    } else {
        1.0
    };
    uniform[12..16].copy_from_slice(&hr_gain.to_ne_bytes());
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        let start = 16 + index * 4;
        uniform[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    let highlight_recovery: u32 = u32::from(context.wbc_highlight_recovery);
    uniform[32..36].copy_from_slice(&highlight_recovery.to_ne_bytes());
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}

impl WhiteBalanceError {
    /// Stable error text shared by ISP preprocess and native executors.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::MissingSource => "missing AsShotNeutral and AsShotWhiteXY",
            Self::InvalidNeutral => "invalid AsShotNeutral components",
            Self::InvalidWhiteXy => "invalid AsShotWhiteXY chromaticity",
            Self::InvalidColorMatrix => "invalid color matrix",
            Self::InvalidCameraWhite => "white point maps to invalid camera values",
            Self::InvalidGains => "white balance gains are not finite and positive",
        }
    }
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
    analog_balance: Option<[f64; 3]>,
) -> Result<[f64; 3], WhiteBalanceError> {
    let [x, y] = white_xy;
    if !x.is_finite() || !y.is_finite() || x <= 0.0 || y <= 0.0 || x + y >= 1.0 {
        return Err(WhiteBalanceError::InvalidWhiteXy);
    }
    if !color_matrix.iter().all(|value| value.is_finite()) {
        return Err(WhiteBalanceError::InvalidColorMatrix);
    }
    let xyz = [x / y, 1.0, (1.0 - x - y) / y];
    let mut camera = [
        color_matrix[0] * xyz[0] + color_matrix[1] * xyz[1] + color_matrix[2] * xyz[2],
        color_matrix[3] * xyz[0] + color_matrix[4] * xyz[1] + color_matrix[5] * xyz[2],
        color_matrix[6] * xyz[0] + color_matrix[7] * xyz[1] + color_matrix[8] * xyz[2],
    ];
    if let Some(balance) = analog_balance {
        if !balance
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            return Err(WhiteBalanceError::InvalidCameraWhite);
        }
        for (camera, balance) in camera.iter_mut().zip(balance) {
            *camera *= balance;
        }
    }
    if !camera.iter().all(|value| value.is_finite() && *value > 0.0) {
        return Err(WhiteBalanceError::InvalidCameraWhite);
    }
    validate_neutral([camera[0] / camera[1], 1.0, camera[2] / camera[1]])
}
