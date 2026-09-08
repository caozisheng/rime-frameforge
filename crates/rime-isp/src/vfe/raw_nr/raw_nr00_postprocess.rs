use crate::operator::{OperatorError, PostprocessContext};

pub(crate) fn run(context: &mut PostprocessContext) -> Result<(), OperatorError> {
    crate::operator::empty_postprocess(context)
}
