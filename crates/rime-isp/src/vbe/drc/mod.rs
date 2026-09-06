mod drc00;
mod drc00_postprocess;
mod drc00_preprocess;
mod drc01;
mod exposure;
mod pipeline;
mod tone;

use crate::operator::OperatorDefinition;
pub use drc00::METHOD_00;
pub use drc01::METHOD_01;
pub use exposure::{
    DrcExposureError, DrcExposureInputs, DrcExposurePolicy, DrcExposureResolution,
    DrcExposureSource, resolve_drc_exposure,
};
pub use tone::{
    DrcLocalStatistics, DrcToneError, LocalToneConfig, LocalToneLutField, ToneLut,
    generate_global_tone_lut, generate_local_tone_lut,
};

pub const DRC_PIPELINE_WGSL: &str = include_str!("drc_pipeline.wgsl");
use rime_core::NodeExecutionMode;
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "drc",
    label: "DRC",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00, METHOD_01],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
