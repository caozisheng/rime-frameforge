mod tintless00;
mod tintless00_postprocess;
mod tintless00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;
pub use tintless00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "tintless",
    label: "TINTLESS",
    mode: NodeExecutionMode::Bypass,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
