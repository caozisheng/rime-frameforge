use super::{lsc00_postprocess, lsc00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "lsc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "mesh_count cfa_phase gain_mesh_headers gain_mesh_entries",
    Some("s0.14"),
    shader(
        "00",
        include_str!("lsc00.wgsl"),
        "lsc_main",
        ShaderBindings {
            input: 0,
            output: 1,
            uniform: Some(2),
        },
    ),
    lsc00_preprocess::run,
    lsc00_postprocess::run,
);
