mod blc_00;

use crate::operator::{OperatorDefinition, OperatorPort};
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};

pub use blc_00::METHOD_00;

pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "blc",
    label: "BLC",
    mode: NodeExecutionMode::Enabled,
    input: OperatorPort {
        domain: SignalDomain::RawBayerSensor,
        format: ResourceFormat::R16Uint,
    },
    output: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    default_method: "00",
    methods: &[METHOD_00],
};
