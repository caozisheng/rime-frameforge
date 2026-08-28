use crate::operator::{method, OperatorMethod, OperatorPort};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: OperatorMethod = method(
    "00",
    "identity_rgba32_main",
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "identity",
);
