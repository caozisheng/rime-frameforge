use super::{tintless00_postprocess, tintless00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "tintless_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "source_extent mesh_extent cfa_pattern gain_clamp cold_start gain_mesh",
    Some("s0.14"),
    shader(
        "00",
        include_str!("tintless00.wgsl"),
        "tintless_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: Some(2),
        },
    ),
    tintless00_preprocess::run,
    tintless00_postprocess::run,
);
