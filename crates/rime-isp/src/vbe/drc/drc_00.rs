use crate::operator::{OperatorMethod, OperatorPort, method};
use rime_core::{ResourceFormat, SignalDomain};
pub const METHOD_00: OperatorMethod = method(
    "00",
    "identity_r32_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "identity",
);
