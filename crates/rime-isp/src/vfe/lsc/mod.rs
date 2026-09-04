mod lsc00;
mod lsc00_postprocess;
mod lsc00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use lsc00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "lsc",
    label: "LSC",
    mode: NodeExecutionMode::Bypass,
    input: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output_rime_q_profile: None,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
