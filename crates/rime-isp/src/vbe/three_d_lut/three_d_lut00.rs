use super::{three_d_lut00_postprocess, three_d_lut00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "identity_rgba32_main",
    OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "identity",
    shader(
        "00",
        include_str!("three_d_lut00.wgsl"),
        "identity_rgba32_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    three_d_lut00_preprocess::run,
    three_d_lut00_postprocess::run,
);
