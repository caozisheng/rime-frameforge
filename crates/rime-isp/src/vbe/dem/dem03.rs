use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_03: MethodManifest = method_manifest(
    "03",
    "demosaic_vng_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern vng_threshold",
    shader(
        "03",
        include_str!("dem03.wgsl"),
        "demosaic_vng_main",
        crate::operator::ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    super::dem03_preprocess::run,
    super::dem03_postprocess::run,
);
