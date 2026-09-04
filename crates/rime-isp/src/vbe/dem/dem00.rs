use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "demosaic_bilinear_main",
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
        "00",
        include_str!("dem00.wgsl"),
        "demosaic_bilinear_main",
        crate::operator::ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    super::dem00_preprocess::run,
    super::dem00_postprocess::run,
);
