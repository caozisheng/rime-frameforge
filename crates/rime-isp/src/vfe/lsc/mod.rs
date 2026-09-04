mod lsc00;
mod lsc00_postprocess;
mod lsc00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use lsc00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "lsc",
    label: "LSC",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
