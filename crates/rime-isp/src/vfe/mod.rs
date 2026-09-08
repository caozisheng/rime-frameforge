pub mod blc;
pub mod cac;
pub mod dbpc;
pub mod lsc;
pub mod raw_nr;
pub mod sbpc;
pub mod sbpc_horizontal;
pub mod tintless;
pub mod white_balance;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &blc::OPERATOR,
    &sbpc_horizontal::OPERATOR,
    &dbpc::OPERATOR,
    &sbpc::OPERATOR,
    &raw_nr::OPERATOR,
    &tintless::OPERATOR,
    &lsc::OPERATOR,
    &white_balance::OPERATOR,
    &cac::OPERATOR,
];
