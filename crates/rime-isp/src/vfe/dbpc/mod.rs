mod dbpc00;
mod dbpc00_postprocess;
mod dbpc00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
pub use dbpc00::METHOD_00;
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "dbpc",
    label: "DBPC",
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
