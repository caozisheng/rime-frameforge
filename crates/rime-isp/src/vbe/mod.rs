pub mod color_correction;
pub mod dem;
pub mod drc;
pub mod gamma;
pub mod pfr;
pub mod rgb_to_yuv;
pub mod three_d_lut;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &drc::OPERATOR,
    &dem::OPERATOR,
    &pfr::OPERATOR,
    &color_correction::OPERATOR,
    &gamma::OPERATOR,
    &three_d_lut::OPERATOR,
    &rgb_to_yuv::OPERATOR,
];
