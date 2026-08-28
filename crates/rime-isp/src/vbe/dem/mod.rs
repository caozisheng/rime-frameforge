mod dem_00;
mod dem_01;
mod dem_02;
mod dem_03;
mod dem_04;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use dem_00::METHOD_00;
pub use dem_01::METHOD_01;
pub use dem_02::METHOD_02;
pub use dem_03::METHOD_03;
pub use dem_04::METHOD_04;
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
    default_method: "00",
    methods: &[METHOD_00, METHOD_01, METHOD_02, METHOD_03, METHOD_04],
};
