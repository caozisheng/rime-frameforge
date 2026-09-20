use crate::{ModuleParameterPacket, OperatorError, PreprocessContext};

use super::{D50_GAINS, validate_source};

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    validate_source(context.width, context.height, context.cfa_pattern).map_err(|error| {
        OperatorError::Preprocess {
            module_id,
            reason: match error {
                crate::LcstStatisticsError::InvalidSourceExtent => "LCST source extent is invalid",
                crate::LcstStatisticsError::InvalidCfaPattern => "LCST CFA pattern is invalid",
                crate::LcstStatisticsError::MissingCfaChannel => {
                    "LCST block is missing a CFA channel"
                }
                _ => "LCST source metadata is invalid",
            },
        }
    })?;
    let mut uniform = [0_u8; 48];
    write_u32(&mut uniform, 0, context.width);
    write_u32(&mut uniform, 4, context.height);
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        write_u32(&mut uniform, 16 + index * 4, value);
    }
    for (index, value) in D50_GAINS.into_iter().enumerate() {
        write_f32(&mut uniform, 32 + index * 4, value);
    }
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}
