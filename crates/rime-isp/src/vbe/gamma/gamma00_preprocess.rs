use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

const DEFAULT_GAMMA: f32 = 2.2;
const IDENTITY_LUT: [f32; 9] = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0];

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let mut uniform = [0_u8; 64];
    uniform[0..4].copy_from_slice(&DEFAULT_GAMMA.to_ne_bytes());
    for (index, value) in IDENTITY_LUT.into_iter().enumerate() {
        let offset = 16 + index * 4;
        uniform[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
