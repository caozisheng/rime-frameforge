use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_01: MethodManifest = method_manifest(
    "01",
    "demosaic_mhc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern",
    shader(
        "01",
        include_str!("dem01.wgsl"),
        "demosaic_mhc_main",
        crate::operator::ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    super::dem01_preprocess::run,
    super::dem01_postprocess::run,
);
