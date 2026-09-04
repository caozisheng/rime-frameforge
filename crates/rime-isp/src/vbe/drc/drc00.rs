use super::{drc00_postprocess, drc00_preprocess};
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
        include_str!("drc00.wgsl"),
        "identity_r32_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    drc00_preprocess::run,
    drc00_postprocess::run,
);
