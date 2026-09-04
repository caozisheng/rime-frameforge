use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    crate::operator::empty_preprocess(context, module_id, method)
}
