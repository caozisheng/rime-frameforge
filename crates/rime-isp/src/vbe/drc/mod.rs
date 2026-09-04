mod drc00;
mod drc00_postprocess;
mod drc00_preprocess;

use crate::operator::OperatorDefinition;
pub use drc00::METHOD_00;
use rime_core::NodeExecutionMode;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "drc",
    label: "DRC",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
