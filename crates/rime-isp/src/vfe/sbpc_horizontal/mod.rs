mod sbpc_horizontal00;
mod sbpc_horizontal00_postprocess;
mod sbpc_horizontal00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;
pub use sbpc_horizontal00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "sbpc_horizontal",
    label: "SBPC-H",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
