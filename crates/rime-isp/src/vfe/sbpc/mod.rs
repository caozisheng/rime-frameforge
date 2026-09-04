mod sbpc00;
mod sbpc00_postprocess;
mod sbpc00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub use sbpc00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "sbpc",
    label: "SBPC",
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
