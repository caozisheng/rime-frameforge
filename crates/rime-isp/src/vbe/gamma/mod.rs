mod gamma00;
mod gamma00_postprocess;
mod gamma00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use gamma00::METHOD_00;

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

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
