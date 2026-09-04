mod dbpc00;
mod dbpc00_postprocess;
mod dbpc00_preprocess;

use crate::operator::OperatorDefinition;
pub use dbpc00::METHOD_00;
use rime_core::NodeExecutionMode;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "dbpc",
    label: "DBPC",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
