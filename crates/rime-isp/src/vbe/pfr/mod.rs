mod pfr_00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use pfr_00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "pfr",
    label: "PFR",
    mode: NodeExecutionMode::Bypass,
    input: OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    default_method: "00",
    methods: &[METHOD_00],
};
