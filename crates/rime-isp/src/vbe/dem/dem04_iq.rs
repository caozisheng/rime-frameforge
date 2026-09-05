#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AhdIqInput {
    pub(crate) scene_brightness_ev: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AhdIqValues {
    pub(crate) ahd_l_threshold: f32,
    pub(crate) ahd_c_threshold_sq: f32,
}

const SCENE_KNOTS: [f64; 6] = [-4.0, 0.0, 4.0, 8.0, 12.0, 16.0];
const L_VALUES: [f64; 6] = [1.0, 1.05, 1.10, 1.16, 1.22, 1.28];
const C_VALUES: [f64; 6] = [3.0, 3.15, 3.30, 3.48, 3.66, 3.84];

#[expect(
    clippy::cast_possible_truncation,
    reason = "AHD thresholds are bounded f32 shader parameters."
)]
pub(crate) fn lookup_default(input: AhdIqInput) -> Result<AhdIqValues, &'static str> {
    if !input.scene_brightness_ev.is_finite() {
        return Err("scene brightness EV must be finite");
    }
    Ok(AhdIqValues {
        ahd_l_threshold: interpolate(&L_VALUES, input.scene_brightness_ev) as f32,
        ahd_c_threshold_sq: interpolate(&C_VALUES, input.scene_brightness_ev) as f32,
    })
}

fn interpolate(values: &[f64; 6], input: f64) -> f64 {
    let value = input.clamp(SCENE_KNOTS[0], SCENE_KNOTS[SCENE_KNOTS.len() - 1]);
    let index = SCENE_KNOTS
        .partition_point(|knot| *knot <= value)
        .saturating_sub(1)
        .min(SCENE_KNOTS.len() - 2);
    let fraction = (value - SCENE_KNOTS[index]) / (SCENE_KNOTS[index + 1] - SCENE_KNOTS[index]);
    values[index] * (1.0 - fraction) + values[index + 1] * fraction
}

#[cfg(test)]
mod tests {
    use super::{AhdIqInput, lookup_default};

    #[test]
    fn lookup_interpolates_direct_scene_brightness_values() {
        let low = lookup_default(AhdIqInput {
            scene_brightness_ev: 0.0,
        })
        .expect("valid input");
        let high = lookup_default(AhdIqInput {
            scene_brightness_ev: 8.0,
        })
        .expect("valid input");
        assert!(high.ahd_l_threshold > low.ahd_l_threshold);
        assert!(high.ahd_c_threshold_sq > low.ahd_c_threshold_sq);
    }

    #[test]
    fn lookup_clamps_scene_brightness() {
        let below = lookup_default(AhdIqInput {
            scene_brightness_ev: -100.0,
        })
        .expect("clamped input");
        let minimum = lookup_default(AhdIqInput {
            scene_brightness_ev: -4.0,
        })
        .expect("minimum input");
        assert_eq!(below, minimum);
    }

    #[test]
    fn rejects_non_finite_scene_brightness() {
        assert!(
            lookup_default(AhdIqInput {
                scene_brightness_ev: f64::NAN
            })
            .is_err()
        );
    }
}
