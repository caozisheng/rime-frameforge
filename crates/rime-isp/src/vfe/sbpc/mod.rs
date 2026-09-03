mod postprocess;
mod preprocess;
mod sbpc_00;
use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub use sbpc_00::METHOD_00;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "sbpc",
    label: "SBPC",
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
        include_str!("sbpc_00.wgsl"),
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
