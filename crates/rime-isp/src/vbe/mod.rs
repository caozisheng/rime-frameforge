pub mod cac;
pub mod color_correction;
pub mod dem;
pub mod drc;
pub mod gamma;
pub mod hr;
pub mod pfr;
pub mod raw_nr;
pub mod rgb_to_yuv;
pub mod three_d_lut;
pub mod white_balance;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &hr::OPERATOR,
    &drc::OPERATOR,
    &cac::OPERATOR,
    &raw_nr::OPERATOR,
    &white_balance::OPERATOR,
    &dem::OPERATOR,
    &pfr::OPERATOR,
    &color_correction::OPERATOR,
    &gamma::OPERATOR,
    &three_d_lut::OPERATOR,
    &rgb_to_yuv::OPERATOR,
];
