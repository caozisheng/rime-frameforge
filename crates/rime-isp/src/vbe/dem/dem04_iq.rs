#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AhdIqInput {
    pub(crate) scene_brightness_ev: f64,
    pub(crate) iso: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AhdIqValues {
    pub(crate) ahd_l_threshold: f32,
    pub(crate) ahd_c_threshold_sq: f32,
}

const SCENE_KNOTS: [f64; 6] = [-4.0, 0.0, 4.0, 8.0, 12.0, 16.0];
const ISO_KNOTS: [f64; 4] = [100.0, 400.0, 1600.0, 6400.0];
const L_VALUES: [[f64; 4]; 6] = [
    [1.0, 1.08, 1.18, 1.30],
    [1.05, 1.14, 1.24, 1.36],
    [1.10, 1.20, 1.31, 1.44],
    [1.16, 1.26, 1.38, 1.52],
    [1.22, 1.33, 1.45, 1.60],
    [1.28, 1.40, 1.53, 1.68],
];
const C_VALUES: [[f64; 4]; 6] = [
    [3.0, 3.24, 3.54, 3.90],
    [3.15, 3.42, 3.72, 4.08],
    [3.30, 3.60, 3.93, 4.32],
    [3.48, 3.78, 4.14, 4.56],
    [3.66, 3.99, 4.35, 4.80],
    [3.84, 4.20, 4.59, 5.04],
];

pub(crate) fn lookup_default(input: AhdIqInput) -> Result<AhdIqValues, &'static str> {
    if !input.iso.is_finite() || input.iso <= 0.0 {
        return Err("ISO must be finite and positive");
    }
    if !input.scene_brightness_ev.is_finite() {
        return Err("scene brightness EV must be finite");
    }
    let (scene_index, scene_fraction) = locate(&SCENE_KNOTS, input.scene_brightness_ev);
    let (iso_index, iso_fraction) = locate_log(&ISO_KNOTS, input.iso);
    Ok(AhdIqValues {
        ahd_l_threshold: bilinear(
            &L_VALUES,
            scene_index,
            scene_fraction,
            iso_index,
            iso_fraction,
        ) as f32,
        ahd_c_threshold_sq: bilinear(
            &C_VALUES,
            scene_index,
            scene_fraction,
            iso_index,
            iso_fraction,
        ) as f32,
    })
}

fn bilinear(
    values: &[[f64; 4]; 6],
    row: usize,
    row_fraction: f64,
    column: usize,
    column_fraction: f64,
) -> f64 {
    let next_row = (row + 1).min(values.len() - 1);
    let next_column = (column + 1).min(values[0].len() - 1);
    let top =
        values[row][column] * (1.0 - column_fraction) + values[row][next_column] * column_fraction;
    let bottom = values[next_row][column] * (1.0 - column_fraction)
        + values[next_row][next_column] * column_fraction;
    top * (1.0 - row_fraction) + bottom * row_fraction
}

fn locate(knots: &[f64], value: f64) -> (usize, f64) {
    let value = value.clamp(knots[0], knots[knots.len() - 1]);
    let index = knots
        .partition_point(|knot| *knot <= value)
        .saturating_sub(1)
        .min(knots.len() - 2);
    (
        index,
        (value - knots[index]) / (knots[index + 1] - knots[index]),
    )
}

fn locate_log(knots: &[f64], value: f64) -> (usize, f64) {
    let log_knots: Vec<f64> = knots.iter().map(|knot| knot.log2()).collect();
    let (index, fraction) = locate(&log_knots, value.log2());
    (index, fraction)
}

#[cfg(test)]
mod tests {
    use super::{AhdIqInput, lookup_default};

    #[test]
    fn lookup_interpolates_scene_brightness_and_log_iso() {
        let low = lookup_default(AhdIqInput {
            scene_brightness_ev: 0.0,
            iso: 100.0,
        })
        .expect("valid input");
        let high = lookup_default(AhdIqInput {
            scene_brightness_ev: 8.0,
            iso: 800.0,
        })
        .expect("valid input");
        assert!(high.ahd_l_threshold > low.ahd_l_threshold);
        assert!(high.ahd_c_threshold_sq > low.ahd_c_threshold_sq);
    }

    #[test]
    fn lookup_clamps_axis_values() {
        let below = lookup_default(AhdIqInput {
            scene_brightness_ev: -100.0,
            iso: 1.0,
        })
        .expect("clamped input");
        let at_min = lookup_default(AhdIqInput {
            scene_brightness_ev: -4.0,
            iso: 100.0,
        })
        .expect("minimum input");
        assert_eq!(below, at_min);
    }

    #[test]
    fn rejects_non_positive_iso() {
        assert!(
            lookup_default(AhdIqInput {
                scene_brightness_ev: 4.0,
                iso: 0.0
            })
            .is_err()
        );
    }
}
