mod raw_nr00;
mod raw_nr00_postprocess;
mod raw_nr00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
pub use raw_nr00::METHOD_00;
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "raw_nr",
    label: "RAW-NR",
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
