mod wbc00;
mod wbc00_postprocess;
mod wbc00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;
pub use wbc00::METHOD_00;
pub use wbc00_preprocess::{
    WhiteBalanceError, WhiteBalanceGains, WhiteBalanceMetadata, white_balance_gains,
};

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "wbc",
    label: "WBC",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};
pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
