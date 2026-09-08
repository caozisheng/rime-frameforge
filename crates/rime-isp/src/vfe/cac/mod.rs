mod cac00;
mod cac00_postprocess;
mod cac00_preprocess;

use crate::operator::OperatorDefinition;
pub use cac00::METHOD_00;
use rime_core::NodeExecutionMode;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "cac",
    label: "CAC",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
