use crate::operator::{method, OperatorMethod, OperatorPort};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_00: OperatorMethod = method(
    "00",
    "demosaic_bilinear_main",
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
