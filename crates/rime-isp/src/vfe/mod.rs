pub mod blc;
pub mod dbpc;
pub mod lsc;
pub mod sbpc;
pub mod sbpc_horizontal;
pub mod tintless;

use crate::operator::OperatorDefinition;

pub const OPERATORS: &[&OperatorDefinition] = &[
    &blc::DEFINITION,
    &sbpc_horizontal::DEFINITION,
    &dbpc::DEFINITION,
    &sbpc::DEFINITION,
    &tintless::DEFINITION,
    &lsc::DEFINITION,
];
