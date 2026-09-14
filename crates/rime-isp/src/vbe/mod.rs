pub mod cac;
pub mod color_reproduce;
pub mod dem;
pub mod drc;
pub mod gamma;
pub mod lsc;
pub mod pfr;
pub mod rgb_to_yuv;
pub mod three_d_lut;
pub mod tintless;
pub mod white_balance;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &tintless::OPERATOR,
    &lsc::OPERATOR,
    &white_balance::OPERATOR,
    &drc::OPERATOR,
    &cac::OPERATOR,
    &dem::OPERATOR,
    &pfr::OPERATOR,
    &color_reproduce::OPERATOR,
    &gamma::OPERATOR,
    &three_d_lut::OPERATOR,
    &rgb_to_yuv::OPERATOR,
];
