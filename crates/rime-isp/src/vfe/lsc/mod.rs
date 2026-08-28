mod lsc_00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use lsc_00::METHOD_00;

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
    default_method: "00",
    methods: &[METHOD_00],
};
