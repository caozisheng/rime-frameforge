mod gamma_00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use gamma_00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "gamma",
    label: "Gamma",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output_rime_q_profile: None,
    default_method: "00",
    methods: &[METHOD_00],
};
