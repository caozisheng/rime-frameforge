use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

/// dem03 (VNG) uniform layout: `cfa_pattern` @0..16, `vng_threshold` @16..20,
/// padding @20..32. The threshold reads the user override from the context
/// and falls back to the module default when absent.
pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let vng_threshold = match context.dem_thresholds {
        Some(thresholds) => super::dem_common::validate_thresholds(thresholds)?.vng_threshold,
        None => super::dem_common::DEFAULT_VNG_THRESHOLD,
    };
    let mut uniform = [0_u8; 32];
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        let start = index * 4;
        uniform[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    uniform[16..20].copy_from_slice(&vng_threshold.to_ne_bytes());
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
