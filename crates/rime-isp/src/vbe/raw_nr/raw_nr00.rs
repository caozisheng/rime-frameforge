use super::{raw_nr00_postprocess, raw_nr00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "identity_r32_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "identity",
    shader(
        "00",
        include_str!("raw_nr00.wgsl"),
        "identity_r32_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    raw_nr00_preprocess::run,
    raw_nr00_postprocess::run,
);
