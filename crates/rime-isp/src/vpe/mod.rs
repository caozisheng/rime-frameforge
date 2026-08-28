pub mod bypass;
pub mod ce;
pub mod mctf;

use crate::operator::OperatorDefinition;

pub const OPERATORS: &[&OperatorDefinition] = &[];

pub const BYPASS_METHOD: &str = bypass::METHOD.method;
