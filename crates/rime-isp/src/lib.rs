#![forbid(unsafe_code)]

mod fused_view;
mod generated;
mod graph;
mod operator;
mod operator_lifecycle;
pub mod primitives;
mod statistics;
pub mod vbe;
pub mod vfe;
pub mod vpe;

pub use fused_view::{
    FUSED_UNIFORM_BYTES, FusedColorReproduce, FusedDemosaicThresholds, FusedGamma,
    FusedHighlightRecovery, FusedUniformRequest, pack_fused_uniforms,
};
pub use graph::{build_normal_graph_presentation, build_normal_manifest};
pub use operator::{
    DemosaicThresholds, FrameIdentity, GainMapParameters, MethodManifest, ModuleParameterPacket,
    ModuleParameterResource, Operator, OperatorDefinition, OperatorError, OperatorPort,
    PostprocessContext, PreprocessContext, ShaderAsset, ShaderBindingAccess, ShaderBindingKind,
    ShaderBindings, ShaderStageAsset, ShaderStageBinding, VignetteRadialParameters,
    empty_postprocess, empty_preprocess, shader_plan,
};
pub use operator_lifecycle::{
    OperatorPhase, OperatorPhaseEvent, PreparedOperatorMethods, complete_operator_methods,
    execute_operator_methods, execute_operator_phases, prepare_operator_methods,
};
pub use statistics::{
    LCST_AVERAGE_BYTES, LCST_AVERAGE_CHANNELS, LCST_AVERAGE_GRID_HEIGHT, LCST_AVERAGE_GRID_WIDTH,
    LCST_AVERAGE_VALUES, LCST_HISTOGRAM_BINS, LCST_HISTOGRAM_BYTES, LCST_HISTOGRAM_GRID_HEIGHT,
    LCST_HISTOGRAM_GRID_WIDTH, LCST_HISTOGRAM_VALUES, LCST_PAYLOAD_BYTES, LcstDecodeFn,
    LcstMethodManifest, LcstProducer, LcstProducerDefinition, LcstStatisticsError,
    LcstStatisticsPacket, StaticLcstProducer, valid_cfa_pattern,
};

pub use generated::{
    render_blc_pipeline_typescript, render_drc_pipeline_typescript,
    render_fused_pipeline_typescript, render_lcst_pipeline_typescript,
    render_lsc_pipeline_typescript, render_normal_graph_presentation_typescript,
    render_normal_graph_quantization_typescript, render_normal_manifest_json,
    render_normal_manifest_typescript, render_segmented_fused_typescript,
    render_tintless_pipeline_typescript, render_wbc_pipeline_typescript,
};
/// Shared fixed-grid quantization and deterministic dither utilities.
pub use rime_quant;

static NORMAL_OPERATORS: &[&dyn Operator] = &[
    &vfe::blc::OPERATOR,
    &vfe::sbpc_horizontal::OPERATOR,
    &vfe::dbpc::OPERATOR,
    &vfe::sbpc::OPERATOR,
    &vfe::raw_nr::OPERATOR,
    &vbe::tintless::OPERATOR,
    &vbe::lsc::OPERATOR,
    &vbe::white_balance::OPERATOR,
    &vbe::drc::OPERATOR,
    &vbe::dem::OPERATOR,
    &vbe::color_reproduce::OPERATOR,
    &vbe::rgb_to_yuv::OPERATOR,
];

#[must_use]
pub fn normal_operators() -> &'static [&'static dyn Operator] {
    NORMAL_OPERATORS
}

#[must_use]
pub fn operator_by_id(id: &str) -> Option<&'static dyn Operator> {
    NORMAL_OPERATORS
        .iter()
        .copied()
        .find(|operator| operator.definition().id == id)
}

#[must_use]
pub fn lcst_producer_by_id(id: &str) -> Option<&'static dyn LcstProducer> {
    (id == vfe::lcst::DEFINITION.id).then_some(&vfe::lcst::PRODUCER as &dyn LcstProducer)
}
