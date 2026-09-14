use crate::operator::{ModuleParameterPacket, OperatorError, PreprocessContext};

/// VNG gradient threshold used when no user override is present (dem03).
pub const DEFAULT_VNG_THRESHOLD: f32 = 1.5;

/// AHD luminance threshold fallback (fused header only); segmented dem04
/// always carries its IQ values in the packet.
pub const DEFAULT_AHD_L_THRESHOLD: f32 = 2.0;

/// AHD chroma threshold squared fallback (fused header only).
pub const DEFAULT_AHD_C_THRESHOLD_SQ: f32 = 4.0;

/// Validates user demosaic threshold overrides: finite, non-negative, and
/// (VNG/AHD-L) strictly positive so gradients are never fully suppressed.
pub(crate) fn validate_thresholds(
    thresholds: crate::operator::DemosaicThresholds,
) -> Result<crate::operator::DemosaicThresholds, OperatorError> {
    let crate::operator::DemosaicThresholds {
        vng_threshold,
        ahd_l_threshold,
        ahd_c_threshold_sq,
    } = thresholds;
    if !vng_threshold.is_finite()
        || vng_threshold <= 0.0
        || !ahd_l_threshold.is_finite()
        || ahd_l_threshold <= 0.0
        || !ahd_c_threshold_sq.is_finite()
        || ahd_c_threshold_sq < 0.0
    {
        return Err(OperatorError::Preprocess {
            module_id: "dem",
            reason: "invalid demosaic thresholds",
        });
    }
    Ok(thresholds)
}

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

#[cfg(test)]
mod tests {
    use super::validate_thresholds;
    use crate::operator::DemosaicThresholds;

    fn valid() -> DemosaicThresholds {
        DemosaicThresholds {
            vng_threshold: 1.5,
            ahd_l_threshold: 2.0,
            ahd_c_threshold_sq: 4.0,
        }
    }

    #[test]
    fn accepts_finite_positive_thresholds() {
        assert!(validate_thresholds(valid()).is_ok());
    }

    #[test]
    fn rejects_non_finite_and_non_positive_values() {
        let mut thresholds = valid();
        thresholds.vng_threshold = f32::NAN;
        assert!(validate_thresholds(thresholds).is_err());
        let mut thresholds = valid();
        thresholds.vng_threshold = 0.0;
        assert!(validate_thresholds(thresholds).is_err());
        let mut thresholds = valid();
        thresholds.ahd_l_threshold = -1.0;
        assert!(validate_thresholds(thresholds).is_err());
        let mut thresholds = valid();
        thresholds.ahd_c_threshold_sq = f32::INFINITY;
        assert!(validate_thresholds(thresholds).is_err());
    }

    #[test]
    fn accepts_zero_ahd_chroma_threshold() {
        let mut thresholds = valid();
        thresholds.ahd_c_threshold_sq = 0.0;
        assert!(validate_thresholds(thresholds).is_ok());
    }
}
