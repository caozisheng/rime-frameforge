mod gamma00;
mod gamma00_postprocess;
mod gamma00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use gamma00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "gamma",
    label: "Gamma",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
