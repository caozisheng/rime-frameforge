pub mod blc;
pub mod dbpc;
pub mod raw_nr;
pub mod sbpc;
pub mod sbpc_horizontal;

use crate::operator::Operator;

pub const OPERATORS: &[&dyn Operator] = &[
    &blc::OPERATOR,
    &sbpc_horizontal::OPERATOR,
    &dbpc::OPERATOR,
    &sbpc::OPERATOR,
    &raw_nr::OPERATOR,
];
