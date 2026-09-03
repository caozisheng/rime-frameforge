use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

pub(crate) fn preprocess(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let mut uniform = [0_u8; 16];
    uniform[0..4].copy_from_slice(&context.black_level.to_ne_bytes());
    uniform[4..8].copy_from_slice(&context.white_level.to_ne_bytes());
    uniform[8..12].copy_from_slice(&context.width.to_ne_bytes());
    uniform[12..16].copy_from_slice(&context.height.to_ne_bytes());
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
