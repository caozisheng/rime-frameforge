#![expect(
    clippy::missing_errors_doc,
    reason = "the typed error enum fully describes exposure-resolution failures"
)]

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrcExposurePolicy {
    Baseline,
    BaselinePlusCaptureBias,
    CaptureBiasOnly,
    CalibratedMetering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrcExposureSource {
    BaselineExposure,
    BaselinePlusCaptureBias,
    CaptureBias,
    CalibratedMetering,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrcExposureInputs {
    pub baseline_exposure_ev: Option<f64>,
    pub exposure_bias_ev: Option<f64>,
    pub brightness_value: Option<f64>,
    pub exposure_time_seconds: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<f64>,
    pub metered_target_ev100: Option<f64>,
    pub profile_adjustment_ev: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrcExposureResolution {
    pub policy: DrcExposurePolicy,
    pub source: DrcExposureSource,
    pub lift_ev: f64,
    pub drc_gain: f64,
    pub capture_ev100: Option<f64>,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DrcExposureError {
    #[error("non-finite DRC exposure metadata")]
    NonFinite,
    #[error("invalid DRC exposure metadata")]
    Invalid,
    #[error("calibrated metering metadata is incomplete")]
    IncompleteMetering,
}

pub fn resolve_drc_exposure(
    inputs: &DrcExposureInputs,
    policy: DrcExposurePolicy,
) -> Result<DrcExposureResolution, DrcExposureError> {
    validate_optional(inputs.baseline_exposure_ev)?;
    validate_optional(inputs.exposure_bias_ev)?;
    validate_optional(inputs.brightness_value)?;
    validate_optional(inputs.exposure_time_seconds)?;
    validate_optional(inputs.f_number)?;
    validate_optional(inputs.iso)?;
    validate_optional(inputs.metered_target_ev100)?;
    if !inputs.profile_adjustment_ev.is_finite() {
        return Err(DrcExposureError::NonFinite);
    }

    let (candidate, source, capture_ev100) = match policy {
        DrcExposurePolicy::Baseline => (
            inputs.baseline_exposure_ev.unwrap_or(0.0),
            inputs
                .baseline_exposure_ev
                .map_or(DrcExposureSource::Fallback, |_| {
                    DrcExposureSource::BaselineExposure
                }),
            None,
        ),
        DrcExposurePolicy::BaselinePlusCaptureBias => {
            match (inputs.baseline_exposure_ev, inputs.exposure_bias_ev) {
                (Some(baseline), Some(bias)) => (
                    baseline - bias,
                    DrcExposureSource::BaselinePlusCaptureBias,
                    None,
                ),
                _ => (0.0, DrcExposureSource::Fallback, None),
            }
        }
        DrcExposurePolicy::CaptureBiasOnly => (
            -inputs.exposure_bias_ev.unwrap_or(0.0),
            inputs
                .exposure_bias_ev
                .map_or(DrcExposureSource::Fallback, |_| {
                    DrcExposureSource::CaptureBias
                }),
            None,
        ),
        DrcExposurePolicy::CalibratedMetering => {
            let exposure_time = inputs
                .exposure_time_seconds
                .ok_or(DrcExposureError::IncompleteMetering)?;
            let f_number = inputs
                .f_number
                .ok_or(DrcExposureError::IncompleteMetering)?;
            let iso = inputs.iso.ok_or(DrcExposureError::IncompleteMetering)?;
            let _brightness = inputs
                .brightness_value
                .ok_or(DrcExposureError::IncompleteMetering)?;
            let target = inputs
                .metered_target_ev100
                .ok_or(DrcExposureError::IncompleteMetering)?;
            if exposure_time <= 0.0 || f_number <= 0.0 || iso <= 0.0 {
                return Err(DrcExposureError::Invalid);
            }
            let capture = (f_number * f_number / exposure_time).log2();
            (
                target - capture + inputs.profile_adjustment_ev,
                DrcExposureSource::CalibratedMetering,
                Some(capture),
            )
        }
    };
    if !candidate.is_finite() {
        return Err(DrcExposureError::NonFinite);
    }
    let lift_ev = candidate.max(0.0);
    let drc_gain = 2.0_f64.powf(lift_ev);
    if !drc_gain.is_finite() {
        return Err(DrcExposureError::Invalid);
    }
    Ok(DrcExposureResolution {
        policy,
        source,
        lift_ev,
        drc_gain,
        capture_ev100,
    })
}

fn validate_optional(value: Option<f64>) -> Result<(), DrcExposureError> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(DrcExposureError::NonFinite);
    }
    Ok(())
}
