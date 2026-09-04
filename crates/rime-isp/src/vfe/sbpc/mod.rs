mod sbpc00;
mod sbpc00_postprocess;
mod sbpc00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;
pub use sbpc00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "sbpc",
    label: "SBPC",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
