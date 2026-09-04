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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IqEffect {
    pub unit: String,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModulationCurve {
    pub id: String,
    pub parameter: String,
    pub values: Vec<f64>,
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

fn validate_entry(address: &str, entry: &ModuleTuningEntry) -> Result<(), ProfileError> {
    if matches!(entry.tuning, TuningMode::Override) {
        let Some(table) = entry.table.as_ref() else {
            return Err(ProfileError::MissingTable {
                address: address.into(),
            });
        };
        for axis in &table.axes {
            if axis.id.is_empty()
                || axis.source.is_empty()
                || axis.unit.is_empty()
                || axis.knots.is_empty()
                || axis.knots.iter().any(|value| !value.is_finite())
                || axis.knots.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(ProfileError::InvalidAxis {
                    address: address.into(),
                    axis: axis.id.clone(),
                });
            }
        }
        for (effect, value) in &table.effects {
            if value.values.len() != table.axes.first().map_or(0, |axis| axis.knots.len())
                || value
                    .values
                    .iter()
                    .any(|item| !item.is_finite() || *item < 0.0)
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
