use super::{drc00_postprocess, drc00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader_plan};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: MethodManifest = method_manifest(
    "00",
    "drc_combine_global_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "drc_gain knee amplifier luma_guard min_ratio max_ratio level_count feature_flags analysis_wbc_gains global_tone_lut",
    None,
    shader_plan(
        "00",
        include_str!("drc_pipeline.wgsl"),
        "drc_combine_global_main",
        ShaderBindings {
            input: 1,
            output: 4,
            uniform: Some(0),
        },
        super::pipeline::GLOBAL_STAGES,
    ),
    drc00_preprocess::run,
    drc00_postprocess::run,
);
