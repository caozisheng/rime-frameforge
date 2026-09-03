mod hr_00;
mod postprocess;
mod preprocess;
use crate::operator::{OperatorDefinition, OperatorPort};
pub use hr_00::METHOD_00;
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "hr",
    label: "HR",
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
    shaders: &[crate::operator::shader(
        "00",
        include_str!("hr_00.wgsl"),
        "identity_r32_main",
        crate::operator::ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    )],
    preprocess: preprocess::run,
    postprocess: postprocess::run,
};
