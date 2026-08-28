use crate::operator::{method, OperatorMethod, OperatorPort};
use rime_core::{ResourceFormat, SignalDomain};

pub const METHOD_04: OperatorMethod = method(
    "04",
    "demosaic_ahd_main",
    OperatorPort {
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
    },
    OperatorPort {
        domain: SignalDomain::LinearRgb,
        format: ResourceFormat::Rgba32Float,
    },
    "cfa_pattern ahd_l_threshold ahd_c_threshold_sq",
);
