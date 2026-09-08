use crate::operator::{OperatorError, PostprocessContext};

#[expect(
    clippy::unnecessary_wraps,
    reason = "WBC currently has no result readback but implements the fallible hook contract"
)]
pub(crate) const fn postprocess(_context: &mut PostprocessContext) -> Result<(), OperatorError> {
    Ok(())
}
