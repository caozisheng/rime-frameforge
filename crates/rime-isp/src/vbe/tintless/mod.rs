mod tintless00;
mod tintless00_postprocess;
mod tintless00_preprocess;
mod tintless_common;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;
pub use tintless_common::{
    TINTLESS_MESH_HEIGHT, TINTLESS_MESH_VALUES, TINTLESS_MESH_WIDTH, TintlessError,
    TintlessGainMesh, estimate_gain_mesh,
};
pub use tintless00::METHOD_00;

pub const TINTLESS_PIPELINE_WGSL: &str = include_str!("tintless00.wgsl");
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "tintless",
    label: "TINTLESS",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
