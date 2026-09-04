mod rgb2yuv00;
mod rgb2yuv00_postprocess;
mod rgb2yuv00_preprocess;

use crate::operator::OperatorDefinition;
use rime_core::NodeExecutionMode;

pub use rgb2yuv00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "rgb2yuv",
    label: "RGB2YUV",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00],
};

pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
