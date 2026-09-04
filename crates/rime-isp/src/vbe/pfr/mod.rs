mod pfr00;
mod pfr00_postprocess;
mod pfr00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use pfr00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "pfr",
    label: "PFR",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
