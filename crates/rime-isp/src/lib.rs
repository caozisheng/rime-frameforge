#![forbid(unsafe_code)]

mod generated;
mod graph;
mod operator;
pub mod preprocess;
pub mod vbe;
pub mod vfe;
pub mod vpe;

pub use graph::{build_normal_graph_presentation, build_normal_manifest};
pub use operator::{OperatorDefinition, OperatorMethod, OperatorPort};

pub use generated::{
    render_normal_graph_presentation_typescript, render_normal_graph_quantization_typescript,
    render_normal_manifest_json, render_normal_manifest_typescript,
};
/// Shared fixed-grid quantization and deterministic dither utilities.
pub use rime_quant;

static NORMAL_OPERATORS: &[&OperatorDefinition] = &[
    &vfe::blc::DEFINITION,
    &vfe::sbpc_horizontal::DEFINITION,
    &vfe::dbpc::DEFINITION,
    &vfe::sbpc::DEFINITION,
    &vfe::tintless::DEFINITION,
    &vfe::lsc::DEFINITION,
    &vbe::hr::DEFINITION,
    &vbe::drc::DEFINITION,
    &vbe::cac::DEFINITION,
    &vbe::raw_nr::DEFINITION,
    &vbe::white_balance::DEFINITION,
    &vbe::dem::DEFINITION,
    &vbe::pfr::DEFINITION,
    &vbe::color_correction::DEFINITION,
    &vbe::gamma::DEFINITION,
    &vbe::three_d_lut::DEFINITION,
    &vbe::rgb_to_yuv::DEFINITION,
];

#[must_use]
pub fn normal_operators() -> &'static [&'static OperatorDefinition] {
    NORMAL_OPERATORS
}
