use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let scene_brightness_ev = context
        .scene_brightness_ev
        .ok_or(OperatorError::Preprocess {
            module_id,
            reason: "AHD IQ requires scene brightness EV",
        })?;
    let iso = context.iso.ok_or(OperatorError::Preprocess {
        module_id,
        reason: "AHD IQ requires ISO",
    })?;
    let values = super::dem04_iq::lookup_default(super::dem04_iq::AhdIqInput {
        scene_brightness_ev,
        iso,
    })
    .map_err(|reason| OperatorError::Preprocess { module_id, reason })?;
    let mut uniform = [0_u8; 32];
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        let start = index * 4;
        uniform[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    uniform[20..24].copy_from_slice(&values.ahd_l_threshold.to_ne_bytes());
    uniform[24..28].copy_from_slice(&values.ahd_c_threshold_sq.to_ne_bytes());
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
