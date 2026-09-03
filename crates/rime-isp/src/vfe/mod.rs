pub mod blc;
pub mod dbpc;
pub mod lsc;
pub mod sbpc;
pub mod sbpc_horizontal;
pub mod tintless;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &blc::OPERATOR,
    &sbpc_horizontal::OPERATOR,
    &dbpc::OPERATOR,
    &sbpc::OPERATOR,
    &tintless::OPERATOR,
    &lsc::OPERATOR,
];
