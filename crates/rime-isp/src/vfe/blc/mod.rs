mod blc00;
mod blc00_postprocess;
mod blc00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use blc00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "blc",
    label: "BLC",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::RawBayerSensor,
        format: ResourceFormat::R16Uint,
    },
    output: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output_rime_q_profile: Some("s0.14"),
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
