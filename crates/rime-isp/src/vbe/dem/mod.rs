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
mod dem04_iq;
mod dem04_postprocess;
mod dem04_preprocess;
mod dem_common;

use crate::operator::OperatorDefinition;
pub use dem00::METHOD_00;
pub use dem01::METHOD_01;
pub use dem02::METHOD_02;
pub use dem03::METHOD_03;
pub use dem04::METHOD_04;
use rime_core::NodeExecutionMode;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "dem",
    label: "DEM",
    mode: NodeExecutionMode::Enabled,
    default_method: "00",
    methods: &[METHOD_00, METHOD_01, METHOD_02, METHOD_03, METHOD_04],
};
pub static OPERATOR: crate::operator::StaticOperator = crate::operator::StaticOperator {
    definition: &DEFINITION,
};
