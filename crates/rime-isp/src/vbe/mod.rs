pub mod color_correction;
pub mod dem;
pub mod drc;
pub mod gamma;
pub mod hr;
pub mod cac;
pub mod pfr;
pub mod raw_nr;
pub mod rgb_to_yuv;
pub mod three_d_lut;
pub mod white_balance;

use crate::operator::OperatorDefinition;

pub const OPERATORS: &[&OperatorDefinition] = &[
    &hr::DEFINITION,
    &drc::DEFINITION,
    &cac::DEFINITION,
    &raw_nr::DEFINITION,
    &white_balance::DEFINITION,
    &dem::DEFINITION,
    &pfr::DEFINITION,
    &color_correction::DEFINITION,
    &gamma::DEFINITION,
    &three_d_lut::DEFINITION,
    &rgb_to_yuv::DEFINITION,
];
