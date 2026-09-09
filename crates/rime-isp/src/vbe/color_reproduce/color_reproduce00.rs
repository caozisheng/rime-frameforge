use super::{color_reproduce00_postprocess, color_reproduce00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "color_reproduce_main",
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "sensor_to_prophoto hs_lut prophoto_to_srgb",
    None,
    shader(
        "00",
        include_str!("color_reproduce00.wgsl"),
        "color_reproduce_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: Some(2),
        },
    ),
    color_reproduce00_preprocess::run,
    color_reproduce00_postprocess::run,
);
