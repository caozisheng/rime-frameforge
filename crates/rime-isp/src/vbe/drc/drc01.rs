use super::{drc00_postprocess, drc00_preprocess};
use crate::operator::{MethodManifest, OperatorPort, ShaderBindings, method_manifest, shader_plan};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_01: MethodManifest = method_manifest(
    "01",
    "drc_combine_local_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "drc_gain knee amplifier luma_guard min_ratio max_ratio level_count feature_flags analysis_wbc_gains global_tone_lut local_tone_lut",
    None,
    shader_plan(
        "01",
        super::DRC_PIPELINE_WGSL,
        "drc_combine_local_main",
        ShaderBindings {
            input: 1,
            output: 4,
            uniform: Some(0),
        },
        super::pipeline::LOCAL_STAGES,
    ),
    drc00_preprocess::run_local,
    drc00_postprocess::run,
);
