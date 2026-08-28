mod rgb_to_yuv_00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use rgb_to_yuv_00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "rgb2yuv",
    label: "RGB2YUV",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::EncodedRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    default_method: "00",
    methods: &[METHOD_00],
};
