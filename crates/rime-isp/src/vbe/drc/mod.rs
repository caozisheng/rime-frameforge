mod drc00;
mod drc00_postprocess;
mod drc00_preprocess;
mod drc01;
mod exposure;
mod pipeline;
mod tone;

/// Optional user-tuned DRC modulation curves, represented as normalized `(x, y)` knots.
#[derive(Clone, Debug, PartialEq)]
pub struct DrcModulationCurves {
    pub edge: Vec<(f64, f64)>,
    pub luma: Vec<(f64, f64)>,
}

use crate::operator::OperatorDefinition;
pub use drc00::METHOD_00;
pub use drc01::METHOD_01;
pub use exposure::{
    DrcExposureError, DrcExposureInputs, DrcExposurePolicy, DrcExposureResolution,
    DrcExposureSource, resolve_drc_exposure,
};
pub use tone::{
    DrcLocalStatistics, DrcToneError, LocalToneConfig, LocalToneLutField, ToneLut,
    build_bayer_local_statistics, generate_global_tone_lut, generate_local_tone_lut,
};

pub const DRC_PIPELINE_WGSL: &str = concat!(
    include_str!("../../primitives/guided_filter_shared.wgsl"),
    include_str!("drc_pipeline.wgsl"),
);
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
