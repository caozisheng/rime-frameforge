mod pfr_00;
mod postprocess;
mod preprocess;

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
    output_rime_q_profile: None,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
    shaders: &[crate::operator::shader(
        "00",
        include_str!("pfr_00.wgsl"),
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
