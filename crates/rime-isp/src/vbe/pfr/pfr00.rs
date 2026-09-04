use super::{pfr00_postprocess, pfr00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "identity_rgba32_main",
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "identity",
    shader(
        "00",
        include_str!("pfr00.wgsl"),
        "identity_rgba32_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    pfr00_preprocess::run,
    pfr00_postprocess::run,
);
