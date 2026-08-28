use crate::operator::{OperatorMethod, OperatorPort, method};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: OperatorMethod = method(
    "00",
    "blc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerSensor,
        format: ResourceFormat::R16Uint,
    },
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    "black_level white_level width height",
);
