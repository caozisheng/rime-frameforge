use crate::operator::{MethodManifest, OperatorPort, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_04: MethodManifest = method_manifest(
    "04",
    "demosaic_ahd_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern ahd_l_threshold ahd_c_threshold_sq",
    shader(
        "04",
        include_str!("dem04.wgsl"),
        "demosaic_ahd_main",
        crate::operator::ShaderBindings {
            input: 1,
            output: 2,
            uniform: Some(0),
        },
    ),
    super::dem04_preprocess::run,
    super::dem04_postprocess::run,
);
