pub mod planckian_locus;
pub mod solver;

mod color_reproduce00;
mod color_reproduce00_postprocess;
mod color_reproduce00_preprocess;

pub use color_reproduce00::METHOD_00;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "color_reproduce",
    label: "Color Reproduce",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
