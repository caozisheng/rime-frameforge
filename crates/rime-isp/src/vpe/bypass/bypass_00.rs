use crate::operator::{OperatorMethod, OperatorPort, method};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD: OperatorMethod = method(
    "00",
    "identity_r32_main",
    OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    OperatorPort {
        domain: SignalDomain::Yuv,
        format: ResourceFormat::Rgba32Float,
    },
    "identity",
);
