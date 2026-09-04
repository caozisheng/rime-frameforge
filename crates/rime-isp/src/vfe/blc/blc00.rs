use super::{blc00_postprocess, blc00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "blc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerSensor,
        format: ResourceFormat::R16Uint,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "black_level white_level width height",
    shader(
        "00",
        include_str!("blc00.wgsl"),
        "blc_main",
        ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    blc00_preprocess::run,
    blc00_postprocess::run,
);
