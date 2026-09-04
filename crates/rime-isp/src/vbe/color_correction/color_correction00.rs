use super::{color_correction00_postprocess, color_correction00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "color_correction_main",
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "ccm",
    None,
    shader(
        "00",
        include_str!("color_correction00.wgsl"),
        "color_correction_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    color_correction00_preprocess::run,
    color_correction00_postprocess::run,
);
