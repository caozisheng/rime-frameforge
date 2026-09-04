use crate::operator::{
    MethodManifest, OperatorPort, ShaderBindings, empty_postprocess, empty_preprocess,
    method_manifest, shader,
};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD: MethodManifest = method_manifest(
    "00",
    "identity_r32_main",
    OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    "identity",
    shader(
        "00",
        "",
        "identity_r32_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    empty_preprocess,
    empty_postprocess,
);
