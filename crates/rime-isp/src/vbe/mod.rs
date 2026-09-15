pub mod color_reproduce;
pub mod dem;
pub mod drc;
pub mod lsc;
pub mod rgb_to_yuv;
pub mod tintless;
pub mod white_balance;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &tintless::OPERATOR,
    &lsc::OPERATOR,
    &white_balance::OPERATOR,
    &drc::OPERATOR,
    &dem::OPERATOR,
    &color_reproduce::OPERATOR,
    &rgb_to_yuv::OPERATOR,
];
