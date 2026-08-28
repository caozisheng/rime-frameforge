mod hr_00;
use crate::operator::{OperatorDefinition, OperatorPort};
pub use hr_00::METHOD_00;
use rime_core::{NodeExecutionMode, ResourceFormat, SignalDomain};
pub const DEFINITION: OperatorDefinition = OperatorDefinition {
    id: "hr",
    label: "HR",
    mode: NodeExecutionMode::Bypass,
    input: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    output: OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    default_method: "00",
    methods: &[METHOD_00],
};
