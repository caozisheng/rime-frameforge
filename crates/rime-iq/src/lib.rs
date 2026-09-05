#![expect(
    clippy::missing_errors_doc,
    reason = "Tuning profile APIs are documented by the surrounding profile contract."
)]
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleCatalogEntry {
    pub address: String,
    pub module_id: String,
    pub method: String,
    pub schema_revision: String,
    pub binding_group: Option<String>,
}

impl ModuleCatalogEntry {
    #[must_use]
    pub fn new(address: &str, module_id: &str, method: &str, schema_revision: &str) -> Self {
        Self {
            address: address.into(),
            module_id: module_id.into(),
            method: method.into(),
            schema_revision: schema_revision.into(),
            binding_group: None,
        }
    }
    #[must_use]
    pub fn with_binding_group(mut self, binding_group: &str) -> Self {
        self.binding_group = Some(binding_group.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TuningProfile {
    pub kind: String,
    pub schema_version: u32,
    pub profile: ProfileMetadata,
    pub pipeline: PipelineMetadata,
    pub camera: CameraMetadata,
    pub modules: BTreeMap<String, ModuleTuningEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileMetadata {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_by: String,
    pub profile_revision: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PipelineMetadata {
    pub graph_id: String,
    pub manifest_revision: String,
    pub base_iq_set: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CameraMetadata {
    pub profile_id: String,
    pub calibration_revision: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleTuningEntry {
    pub module_id: String,
    pub method: String,
    #[serde(default)]
    pub binding_group: Option<String>,
    pub tuning: TuningMode,
    #[serde(default)]
    pub table: Option<ModuleIqTable>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TuningMode {
    Inherit,
    Override,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleIqTable {
    pub schema_version: u32,
    pub parameter_schema_revision: String,
    pub axes: Vec<IqAxis>,
    pub effects: BTreeMap<String, IqEffect>,
    #[serde(default)]
    pub modulation_curves: Vec<ModulationCurve>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IqAxis {
    pub id: String,
    pub source: String,
    pub unit: String,
    #[serde(default)]
    pub coordinate_transform: Option<String>,
    pub knots: Vec<f64>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IqInterpolation {
    Linear,
    Bezier,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IqCombine {
    Direct,
    Multiply,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IqLut {
    pub axis: String,
    pub interpolation: IqInterpolation,
    pub knots: Vec<f64>,
    pub values: Vec<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IqEffect {
    pub unit: String,
    pub axis: String,
    pub combine: IqCombine,
    pub interpolation: IqInterpolation,
    pub knots: Vec<f64>,
    pub values: Vec<f64>,
    #[serde(default)]
    pub factors: Vec<IqLut>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModulationCurve {
    pub id: String,
    pub parameter: String,
    pub values: Vec<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedIqEffect {
    pub direct_value: f64,
    pub factor_values: Vec<f64>,
    pub final_value: f64,
}

#[derive(Clone, Debug)]
pub struct ResolvedProfile {
    profile: TuningProfile,
}

#[derive(Debug, Error, PartialEq)]
pub enum ProfileError {
    #[error("profile YAML parse failed: {0}")]
    Parse(String),
    #[error("profile YAML serialization failed: {0}")]
    Serialize(String),
    #[error("unknown module address `{address}`")]
    UnknownModule { address: String },
    #[error("missing module address `{address}`")]
    MissingModule { address: String },
    #[error("module `{address}` metadata does not match catalog")]
    ModuleMismatch { address: String },
    #[error("module `{address}` override requires a complete table")]
    MissingTable { address: String },
    #[error("module `{address}` table has invalid shape for effect `{effect}`")]
    InvalidTableShape { address: String, effect: String },
    #[error("module `{address}` has invalid axis `{axis}`")]
    InvalidAxis { address: String, axis: String },
    #[error("effect `{effect}` must declare a direct principal axis")]
    MissingPrincipalAxis { effect: String },
    #[error("effect `{effect}` references unknown axis `{axis}`")]
    UnknownAxis { effect: String, axis: String },
    #[error("effect `{effect}` requires finite coordinate `{axis}`")]
    MissingCoordinate { effect: String, axis: String },
}

impl TuningProfile {
    pub fn from_yaml(source: &str) -> Result<Self, ProfileError> {
        serde_yaml::from_str(source).map_err(|error| ProfileError::Parse(error.to_string()))
    }
    pub fn to_yaml(&self) -> Result<String, ProfileError> {
        serde_yaml::to_string(self).map_err(|error| ProfileError::Serialize(error.to_string()))
    }
    pub fn resolve(self, catalog: &[ModuleCatalogEntry]) -> Result<ResolvedProfile, ProfileError> {
        if self.kind != "rime.tuning_profile" || self.schema_version != 1 {
            return Err(ProfileError::Parse(
                "unsupported profile kind or schema version".into(),
            ));
        }
        for address in self.modules.keys() {
            if !catalog.iter().any(|entry| entry.address == *address) {
                return Err(ProfileError::UnknownModule {
                    address: address.clone(),
                });
            }
        }
        for entry in catalog {
            let Some(module) = self.modules.get(&entry.address) else {
                return Err(ProfileError::MissingModule {
                    address: entry.address.clone(),
                });
            };
            if module.module_id != entry.module_id
                || module.method != entry.method
                || module.binding_group != entry.binding_group
            {
                return Err(ProfileError::ModuleMismatch {
                    address: entry.address.clone(),
                });
            }
            validate_entry(&entry.address, module)?;
        }
        Ok(ResolvedProfile { profile: self })
    }
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile.id
    }
    #[must_use]
    pub const fn profile_revision(&self) -> u64 {
        self.profile.profile_revision
    }
    #[must_use]
    pub fn modules(&self) -> &BTreeMap<String, ModuleTuningEntry> {
        &self.modules
    }
}

impl ResolvedProfile {
    #[must_use]
    pub fn profile_id(&self) -> &str {
        self.profile.profile_id()
    }
    #[must_use]
    pub fn module(&self, address: &str) -> Option<&ModuleTuningEntry> {
        self.profile.modules.get(address)
    }
}
impl ModuleTuningEntry {
    #[must_use]
    pub const fn is_override(&self) -> bool {
        matches!(self.tuning, TuningMode::Override)
    }
    #[must_use]
    pub const fn is_inherit(&self) -> bool {
        matches!(self.tuning, TuningMode::Inherit)
    }
}

impl IqAxis {
    fn validate(&self, address: &str) -> Result<(), ProfileError> {
        if self.id.is_empty()
            || self.source.is_empty()
            || self.unit.is_empty()
            || self.knots.is_empty()
            || self.knots.iter().any(|value| !value.is_finite())
            || self.knots.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ProfileError::InvalidAxis {
                address: address.into(),
                axis: self.id.clone(),
            });
        }
        Ok(())
    }
}
impl IqLut {
    fn validate(&self, effect: &str) -> Result<(), ProfileError> {
        if self.axis.is_empty()
            || self.knots.len() != self.values.len()
            || self.knots.len() < 2
            || self.knots.iter().any(|value| !value.is_finite())
            || self.knots.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .values
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(ProfileError::InvalidTableShape {
                address: "effect".into(),
                effect: effect.into(),
            });
        }
        Ok(())
    }
}
impl IqEffect {
    pub fn resolve(
        &self,
        effect: &str,
        coordinates: &BTreeMap<String, f64>,
        axis_catalog: &[IqAxis],
    ) -> Result<ResolvedIqEffect, ProfileError> {
        if self.combine != IqCombine::Direct {
            return Err(ProfileError::MissingPrincipalAxis {
                effect: effect.into(),
            });
        }
        let Some(principal_axis) = axis_catalog
            .iter()
            .find(|candidate| candidate.id == self.axis)
        else {
            return Err(ProfileError::UnknownAxis {
                effect: effect.into(),
                axis: self.axis.clone(),
            });
        };
        let Some(coordinate) = coordinates
            .get(&principal_axis.id)
            .copied()
            .filter(|value| value.is_finite())
        else {
            return Err(ProfileError::MissingCoordinate {
                effect: effect.into(),
                axis: principal_axis.id.clone(),
            });
        };
        let direct_value = interpolate(&self.knots, &self.values, coordinate);
        let mut factor_values = Vec::with_capacity(self.factors.len());
        let mut final_value = direct_value;
        for factor in &self.factors {
            factor.validate(effect)?;
            let Some(value) = coordinates
                .get(&factor.axis)
                .copied()
                .filter(|value| value.is_finite())
            else {
                return Err(ProfileError::MissingCoordinate {
                    effect: effect.into(),
                    axis: factor.axis.clone(),
                });
            };
            let factor_value = interpolate(&factor.knots, &factor.values, value);
            factor_values.push(factor_value);
            final_value *= factor_value;
        }
        Ok(ResolvedIqEffect {
            direct_value,
            factor_values,
            final_value,
        })
    }
}

fn interpolate(knots: &[f64], values: &[f64], value: f64) -> f64 {
    let value = value.clamp(knots[0], knots[knots.len() - 1]);
    let index = knots
        .partition_point(|knot| *knot <= value)
        .saturating_sub(1)
        .min(knots.len() - 2);
    let fraction = (value - knots[index]) / (knots[index + 1] - knots[index]);
    values[index] * (1.0 - fraction) + values[index + 1] * fraction
}

fn validate_entry(address: &str, entry: &ModuleTuningEntry) -> Result<(), ProfileError> {
    if matches!(entry.tuning, TuningMode::Override) {
        let Some(table) = entry.table.as_ref() else {
            return Err(ProfileError::MissingTable {
                address: address.into(),
            });
        };
        for axis in &table.axes {
            axis.validate(address)?;
        }
        for (effect, value) in &table.effects {
            if value.axis.is_empty()
                || value.knots.len() != value.values.len()
                || value.knots.len() < 2
                || value.knots.iter().any(|item| !item.is_finite())
                || value.knots.windows(2).any(|pair| pair[0] >= pair[1])
                || value
                    .values
                    .iter()
                    .any(|item| !item.is_finite() || *item < 0.0)
                || !table.axes.iter().any(|axis| axis.id == value.axis)
                || value.factors.iter().any(|factor| {
                    factor.axis.is_empty()
                        || factor.knots.len() != factor.values.len()
                        || factor.knots.len() < 2
                })
            {
                return Err(ProfileError::InvalidTableShape {
                    address: address.into(),
                    effect: effect.clone(),
                });
            }
        }
    }
    Ok(())
}
