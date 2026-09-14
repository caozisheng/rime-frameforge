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
    let mut values = super::dem04_iq::lookup_default(super::dem04_iq::AhdIqInput {
        scene_brightness_ev,
    })
    .map_err(|reason| OperatorError::Preprocess { module_id, reason })?;
    if let Some(thresholds) = context.dem_thresholds {
        let thresholds = super::dem_common::validate_thresholds(thresholds)?;
        values.ahd_l_threshold = thresholds.ahd_l_threshold;
        values.ahd_c_threshold_sq = thresholds.ahd_c_threshold_sq;
    }
    let mut uniform = [0_u8; 32];
    for (index, value) in context.cfa_pattern.into_iter().enumerate() {
        let start = index * 4;
        uniform[start..start + 4].copy_from_slice(&value.to_ne_bytes());
    }
    uniform[20..24].copy_from_slice(&values.ahd_l_threshold.to_ne_bytes());
    uniform[24..28].copy_from_slice(&values.ahd_c_threshold_sq.to_ne_bytes());
    ModuleParameterPacket::new(module_id, method, context.identity, &uniform)
}
