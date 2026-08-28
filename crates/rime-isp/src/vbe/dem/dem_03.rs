use crate::operator::{method, OperatorMethod, OperatorPort};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_03: OperatorMethod = method(
    "03",
    "demosaic_vng_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern vng_threshold",
);
