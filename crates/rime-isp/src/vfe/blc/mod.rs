mod blc00;
mod blc00_postprocess;
mod blc00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use blc00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "blc",
    label: "BLC",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
