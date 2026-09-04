mod dem00;
mod dem00_postprocess;
mod dem00_preprocess;
mod dem01;
mod dem01_postprocess;
mod dem01_preprocess;
mod dem02;
mod dem02_postprocess;
mod dem02_preprocess;
mod dem03;
mod dem03_postprocess;
mod dem03_preprocess;
mod dem04;
mod dem04_postprocess;
mod dem04_preprocess;
mod dem_common;

use crate::operator::{OperatorDefinition, OperatorPort};
pub use dem00::METHOD_00;
pub use dem01::METHOD_01;
pub use dem02::METHOD_02;
pub use dem03::METHOD_03;
pub use dem04::METHOD_04;
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "dem",
    label: "DEM",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    output_rime_q_profile: Some("s0.12"),
    default_method: "00",
    methods: &[METHOD_00, METHOD_01, METHOD_02, METHOD_03, METHOD_04],
};
pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
