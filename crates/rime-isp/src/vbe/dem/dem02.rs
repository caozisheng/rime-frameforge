use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_02: MethodManifest = method_manifest(
    "02",
    "demosaic_ppg_main",
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
        "02",
        include_str!("dem02.wgsl"),
        "demosaic_ppg_main",
        crate::operator::ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    super::dem02_preprocess::run,
    super::dem02_postprocess::run,
);
