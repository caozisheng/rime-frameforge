use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

pub(crate) fn preprocess(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let mut uniform = [0_u8; 32];
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        let start = index * 4;
        uniform[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
