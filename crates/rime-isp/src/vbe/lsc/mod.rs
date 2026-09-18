mod lsc00;
mod lsc00_postprocess;
mod lsc00_preprocess;
mod lsc_common;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use crate::operator::{GainMapParameters, VignetteRadialParameters};
pub use lsc_common::{MeshGeometry, VIGNETTE_MESH_POINTS, mesh_gain};
pub use lsc00::METHOD_00;

pub const LSC_PIPELINE_WGSL: &str = include_str!("lsc00.wgsl");

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "lsc",
    label: "LSC",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
