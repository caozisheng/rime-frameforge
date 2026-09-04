#![forbid(unsafe_code)]

mod generated;
mod graph;
mod operator;
pub mod vbe;
pub mod vfe;
pub mod vpe;

pub use graph::{build_normal_graph_presentation, build_normal_manifest};
pub use operator::{
    FrameIdentity, MethodManifest, ModuleParameterPacket, Operator, OperatorDefinition,
    OperatorError, OperatorPort, PostprocessContext, PreprocessContext, ShaderAsset,
    ShaderBindings, empty_postprocess, empty_preprocess,
};

pub use generated::{
    render_normal_graph_presentation_typescript, render_normal_graph_quantization_typescript,
    render_normal_manifest_json, render_normal_manifest_typescript,
};
/// Shared fixed-grid quantization and deterministic dither utilities.
pub use rime_quant;

static NORMAL_OPERATORS: &[&dyn Operator] = &[
    &vfe::blc::OPERATOR,
    &vfe::sbpc_horizontal::OPERATOR,
    &vfe::dbpc::OPERATOR,
    &vfe::sbpc::OPERATOR,
    &vfe::tintless::OPERATOR,
    &vfe::lsc::OPERATOR,
    &vbe::hr::OPERATOR,
    &vbe::drc::OPERATOR,
    &vbe::cac::OPERATOR,
    &vbe::raw_nr::OPERATOR,
    &vbe::white_balance::OPERATOR,
    &vbe::dem::OPERATOR,
    &vbe::pfr::OPERATOR,
    &vbe::color_correction::OPERATOR,
    &vbe::gamma::OPERATOR,
    &vbe::three_d_lut::OPERATOR,
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
