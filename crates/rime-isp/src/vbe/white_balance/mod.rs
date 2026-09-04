mod wbc00_postprocess;
mod wbc00_preprocess;
mod white_balance00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub use wbc00_preprocess::{
    WhiteBalanceError, WhiteBalanceGains, WhiteBalanceMetadata, white_balance_gains,
};
pub use white_balance00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "wbc",
    label: "WBC",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output_rime_q_profile: Some("s0.12"),
    default_method: "00",
    methods: &[METHOD_00],
};
pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
