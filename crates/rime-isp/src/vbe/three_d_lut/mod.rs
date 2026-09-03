mod postprocess;
mod preprocess;
mod three_d_lut_00;
use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub use three_d_lut_00::METHOD_00;
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
    shaders: &[crate::operator::shader(
        "00",
        include_str!("three_d_lut_00.wgsl"),
        "identity_rgba32_main",
        crate::operator::ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    )],
    preprocess: preprocess::run,
    postprocess: postprocess::run,
};
