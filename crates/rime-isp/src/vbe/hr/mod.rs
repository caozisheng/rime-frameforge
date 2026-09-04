mod hr00;
mod hr00_postprocess;
mod hr00_preprocess;

use crate::operator::OperatorDefinition;
pub use hr00::METHOD_00;
use rime_core::NodeExecutionMode;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "hr",
    label: "HR",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
