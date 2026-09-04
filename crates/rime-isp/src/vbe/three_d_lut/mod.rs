mod three_d_lut00;
mod three_d_lut00_postprocess;
mod three_d_lut00_preprocess;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub use three_d_lut00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "three_d_lut",
    label: "3D LUT 17³",
    mode: NodeExecutionMode::Bypass,
    input: OperatorPort {
        domain: SignalDomain::EncodedRgb,
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
