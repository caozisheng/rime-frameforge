#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_LUMINANCE_CONSTANT_K: f64 = 12.5;
const DEFAULT_ILLUMINANCE_CONSTANT_C: f64 = 250.0;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneInput {
    pub frame_index: u64,
    pub aperture_f_number: Option<f64>,
    pub exposure_time_seconds: Option<f64>,
    pub iso: Option<f64>,
    pub brightness_value: Option<f64>,
    pub scene_brightness_ev: Option<f64>,
    pub scene_luminance_cd_m2: Option<f64>,
    pub scene_illuminance_lux: Option<f64>,
    pub exposure_bias_ev: Option<f64>,
    pub analog_gain: Option<f64>,
    pub digital_gain: Option<f64>,
    pub cct_kelvin: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneBrightnessSource {
    Measured,
    Calibrated,
    Estimated,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneProfile {
    #[serde(default = "default_luminance_constant_k")]
    pub luminance_constant_k: f64,
    #[serde(default = "default_illuminance_constant_c")]
    pub illuminance_constant_c: f64,
}

impl SceneProfile {
    fn validate(&self) -> Result<(), SceneError> {
        if !self.luminance_constant_k.is_finite() || self.luminance_constant_k <= 0.0 {
            return Err(SceneError::InvalidProfileConstant {
                name: "luminance constant K",
            });
        }
        if !self.illuminance_constant_c.is_finite() || self.illuminance_constant_c <= 0.0 {
            return Err(SceneError::InvalidProfileConstant {
                name: "illuminance constant C",
            });
        }
        Ok(())
    }
}

impl Default for SceneProfile {
    fn default() -> Self {
        Self {
            luminance_constant_k: DEFAULT_LUMINANCE_CONSTANT_K,
            illuminance_constant_c: DEFAULT_ILLUMINANCE_CONSTANT_C,
        }
    }
}

impl Default for SceneBrightnessSource {
    fn default() -> Self {
        Self::Unavailable
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneBrightness {
    pub ev_apex: Option<f64>,
    pub luminance_cd_m2: Option<f64>,
    pub illuminance_lux: Option<f64>,
    pub source: SceneBrightnessSource,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneMeta {
    pub frame_index: u64,
    pub capture_ev100: Option<f64>,
    pub scene_brightness: SceneBrightness,
    pub exposure_deviation_ev: Option<f64>,
    pub iso: Option<f64>,
    pub analog_gain: Option<f64>,
    pub digital_gain: Option<f64>,
    pub cct_kelvin: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneLabel {
    id: String,
    min_ev: f64,
    max_ev: f64,
}

impl SceneLabel {
    pub fn new(id: impl Into<String>, min_ev: f64, max_ev: f64) -> Result<Self, SceneError> {
        let id = id.into();
        if id.is_empty() {
            return Err(SceneError::InvalidLabelId);
        }
        if !min_ev.is_finite() || !max_ev.is_finite() || min_ev >= max_ev {
            return Err(SceneError::InvalidLabelRange);
        }
        Ok(Self { id, min_ev, max_ev })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn contains(&self, ev: f64) -> bool {
        self.min_ev <= ev && ev < self.max_ev
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneLabelSet {
    labels: Vec<SceneLabel>,
}

impl SceneLabelSet {
    pub fn new(labels: Vec<SceneLabel>) -> Result<Self, SceneError> {
        for (index, left) in labels.iter().enumerate() {
            if labels
                .iter()
                .skip(index + 1)
                .any(|right| left.min_ev < right.max_ev && right.min_ev < left.max_ev)
            {
                return Err(SceneError::OverlappingLabels);
            }
        }
        Ok(Self { labels })
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum SceneError {
    #[error("aperture f-number must be finite and positive")]
    InvalidAperture,
    #[error("exposure time must be finite and positive")]
    InvalidExposureTime,
    #[error("ISO must be finite and positive")]
    InvalidIso,
    #[error("scene metadata value `{name}` must be finite")]
    NonFiniteValue { name: &'static str },
    #[error("scene metadata value `{name}` must be positive")]
    NonPositiveValue { name: &'static str },
    #[error("aperture and exposure time must be provided together")]
    IncompleteExposure,
    #[error("{name} must be finite and positive")]
    InvalidProfileConstant { name: &'static str },
    #[error("scene label id must not be empty")]
    InvalidLabelId,
    #[error("scene label range must be finite and min < max")]
    InvalidLabelRange,
    #[error("scene label ranges must not overlap")]
    OverlappingLabels,
    #[error("scene brightness cannot be derived from the supplied metadata")]
    SceneBrightnessUnavailable,
}

#[must_use]
pub fn ev100_capture(input: &SceneInput) -> Result<f64, SceneError> {
    let Some(aperture) = input.aperture_f_number else {
        return if input.exposure_time_seconds.is_some() {
            Err(SceneError::IncompleteExposure)
        } else {
            Err(SceneError::InvalidAperture)
        };
    };
    let Some(exposure_time) = input.exposure_time_seconds else {
        return Err(SceneError::IncompleteExposure);
    };
    validate_positive(aperture, SceneError::InvalidAperture)?;
    validate_positive(exposure_time, SceneError::InvalidExposureTime)?;
    Ok((aperture * aperture / exposure_time).log2())
}

pub fn derive_scene_meta(
    input: &SceneInput,
    profile: &SceneProfile,
) -> Result<SceneMeta, SceneError> {
    profile.validate()?;
    validate_optional_positive(input.iso, SceneError::InvalidIso)?;
    validate_optional_finite(input.brightness_value, "brightness value")?;
    validate_optional_finite(input.scene_brightness_ev, "scene brightness EV")?;
    validate_optional_positive(input.scene_luminance_cd_m2, SceneError::NonPositiveValue { name: "scene luminance" })?;
    validate_optional_positive(input.scene_illuminance_lux, SceneError::NonPositiveValue { name: "scene illuminance" })?;
    validate_optional_finite(input.exposure_bias_ev, "exposure bias")?;
    validate_optional_positive(input.analog_gain, SceneError::NonPositiveValue { name: "analog gain" })?;
    validate_optional_positive(input.digital_gain, SceneError::NonPositiveValue { name: "digital gain" })?;
    validate_optional_positive(input.cct_kelvin, SceneError::NonPositiveValue { name: "CCT" })?;

    let capture_ev100 = match (input.aperture_f_number, input.exposure_time_seconds) {
        (None, None) => None,
        _ => Some(ev100_capture(input)?),
    };
    let scene_brightness = derive_scene_brightness(input, profile, capture_ev100)?;

    Ok(SceneMeta {
        frame_index: input.frame_index,
        capture_ev100,
        scene_brightness,
        exposure_deviation_ev: input.exposure_bias_ev,
        iso: input.iso,
        analog_gain: input.analog_gain,
        digital_gain: input.digital_gain,
        cct_kelvin: input.cct_kelvin,
    })
}

#[must_use]
pub fn classify_scene<'a>(meta: &SceneMeta, labels: &'a SceneLabelSet) -> Option<&'a SceneLabel> {
    meta.scene_brightness
        .ev_apex
        .and_then(|ev| labels.labels.iter().find(|label| label.contains(ev)))
}

fn derive_scene_brightness(
    input: &SceneInput,
    profile: &SceneProfile,
    capture_ev100: Option<f64>,
) -> Result<SceneBrightness, SceneError> {
    let ev_apex = if let Some(value) = input.brightness_value {
        Some(value)
    } else if let Some(value) = input.scene_brightness_ev {
        Some(value)
    } else if let (Some(capture_ev), Some(iso)) = (capture_ev100, input.iso) {
        Some(capture_ev - (iso / 100.0).log2() + input.exposure_bias_ev.unwrap_or(0.0))
    } else {
        None
    };

    let source = if input.brightness_value.is_some() {
        SceneBrightnessSource::Measured
    } else if input.scene_brightness_ev.is_some()
        || input.scene_luminance_cd_m2.is_some()
        || input.scene_illuminance_lux.is_some()
    {
        SceneBrightnessSource::Calibrated
    } else if ev_apex.is_some() {
        SceneBrightnessSource::Estimated
    } else {
        SceneBrightnessSource::Unavailable
    };

    if ev_apex.is_none()
        && input.scene_luminance_cd_m2.is_none()
        && input.scene_illuminance_lux.is_none()
    {
        return Err(SceneError::SceneBrightnessUnavailable);
    }

    let luminance_cd_m2 = input
        .scene_luminance_cd_m2
        .or_else(|| ev_apex.map(|ev| profile.luminance_constant_k * 2.0_f64.powf(ev)));
    let illuminance_lux = input
        .scene_illuminance_lux
        .or_else(|| ev_apex.map(|ev| profile.illuminance_constant_c * 2.0_f64.powf(ev)));

    Ok(SceneBrightness {
        ev_apex,
        luminance_cd_m2,
        illuminance_lux,
        source,
    })
}

fn validate_positive(value: f64, error: SceneError) -> Result<(), SceneError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(error);
    }
    Ok(())
}

fn validate_optional_positive(value: Option<f64>, error: SceneError) -> Result<(), SceneError> {
    if let Some(value) = value {
        validate_positive(value, error)?;
    }
    Ok(())
}

fn validate_optional_finite(value: Option<f64>, name: &'static str) -> Result<(), SceneError> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(SceneError::NonFiniteValue { name });
    }
    Ok(())
}

const fn default_luminance_constant_k() -> f64 {
    DEFAULT_LUMINANCE_CONSTANT_K
}

const fn default_illuminance_constant_c() -> f64 {
    DEFAULT_ILLUMINANCE_CONSTANT_C
}
