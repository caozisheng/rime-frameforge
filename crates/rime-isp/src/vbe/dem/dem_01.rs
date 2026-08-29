use crate::operator::{OperatorMethod, OperatorPort, method};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_01: OperatorMethod = method(
    "01",
    "demosaic_mhc_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern",
);
