use super::{rgb2yuv00_postprocess, rgb2yuv00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "rgb2yuv_main",
    OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    "bt709",
    Some("s0.10"),
    shader(
        "00",
        include_str!("rgb2yuv00.wgsl"),
        "rgb2yuv_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: None,
        },
    ),
    rgb2yuv00_preprocess::run,
    rgb2yuv00_postprocess::run,
);
