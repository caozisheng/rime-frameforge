use super::{gamma00_postprocess, gamma00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "gamma_main",
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "gamma gamma_lut",
    None,
    shader(
        "00",
        include_str!("gamma00.wgsl"),
        "gamma_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: Some(2),
        },
    ),
    gamma00_preprocess::run,
    gamma00_postprocess::run,
);
