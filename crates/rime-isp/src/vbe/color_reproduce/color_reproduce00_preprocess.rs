//! Color reproduce preprocess: solves the sensor->ProPhoto and
//! ProPhoto->sRGB matrices, interpolates the HSV calibration LUT, and packs
//! the invocation-frozen parameter packet.
#![expect(
    clippy::cast_possible_truncation,
    reason = "f64 solutions are narrowed to the GPU f32 parameter contract"
)]
use crate::operator::{
    ModuleParameterPacket, ModuleParameterResource, OperatorError, PreprocessContext,
};

use super::solver::{ColorReproduceError, ColorReproduceInputs, Matrix3, solve_color_reproduce};

pub(crate) fn run(
    context: &PreprocessContext,
    module_id: &'static str,
    method: &'static str,
) -> Result<ModuleParameterPacket, OperatorError> {
    let inputs = collect_inputs(context)
        .map_err(|reason| OperatorError::Preprocess { module_id, reason })?;
    let solution = solve_color_reproduce(&inputs).map_err(map_solver_error(module_id))?;

    // HSV calibration: interpolate the two profile tables with the same
    // converged illuminant weights that produced the matrices. A frame
    // without a complete, size-consistent HSV map bypasses the lookup
    // (identity) instead of failing — matrices still ship.
    let (dims, hsv_lut) = match context.profile_hue_sat_map_dims {
        None => ([1u32, 1, 1], None),
        Some(dims) => {
            let [hue, saturation, value] = dims;
            let expected = 3 * hue as usize * saturation as usize * value as usize;
            let lut = match (
                context.profile_hue_sat_map_data1.as_ref(),
                context.profile_hue_sat_map_data2.as_ref(),
            ) {
                (Some(data1), None)
                    if context.color_matrix2.is_none() && data1.len() == expected =>
                {
                    interpolate(data1, data1, solution.weight1, solution.weight2)
                }
                (Some(data1), Some(data2))
                    if data1.len() == expected && data2.len() == expected =>
                {
                    interpolate(data1, data2, solution.weight1, solution.weight2)
                }
                _ => Vec::new(),
            };
            if lut.is_empty() {
                ([1u32, 1, 1], None)
            } else {
                (dims, Some(lut))
            }
        }
    };

    // Uniform (binding 2): hue/sat/value divs plus the enable flag.
    let mut uniform = [0_u8; 16];
    for (slot, value) in dims.into_iter().enumerate() {
        uniform[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    let enable: u32 = u32::from(hsv_lut.is_some());
    uniform[12..16].copy_from_slice(&enable.to_ne_bytes());
    let mut packet = ModuleParameterPacket::new(module_id, method, context.identity, &uniform)?;

    // Matrices resource: 18 row-major f32 values.
    let mut matrices = Vec::with_capacity(18);
    for row in 0..3 {
        for col in 0..3 {
            matrices.push(solution.sensor_to_prophoto[row][col] as f32);
        }
    }
    for row in 0..3 {
        for col in 0..3 {
            matrices.push(solution.prophoto_to_srgb[row][col] as f32);
        }
    }
    packet.push_resource(ModuleParameterResource::new(
        "cr_matrices",
        [18, 1, 1],
        encode_f32(&matrices),
    ))?;

    // The shader always declares the cr_hsv_lut storage binding; a bypassed
    // lookup ships a placeholder (the enable flag keeps it unread) so the
    // bind group layout matches on every frame.
    let lut_bytes = hsv_lut
        .as_ref()
        .map_or_else(|| encode_f32(&[0.0, 1.0, 1.0]), |lut| encode_f32(lut));
    let lut_extent = hsv_lut
        .as_ref()
        .map_or([1, 1, 1], |lut| [lut.len() as u32 / 3, 1, 1]);
    packet.push_resource(ModuleParameterResource::new(
        "cr_hsv_lut",
        lut_extent,
        lut_bytes,
    ))?;
    Ok(packet)
}

fn interpolate(data1: &[f32], data2: &[f32], weight1: f64, weight2: f64) -> Vec<f32> {
    data1
        .iter()
        .zip(data2)
        .map(|(a, b)| (weight1 * f64::from(*a) + weight2 * f64::from(*b)) as f32)
        .collect()
}

fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

fn matrix3(values: [f64; 9]) -> Matrix3 {
    [
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ]
}

fn collect_inputs(context: &PreprocessContext) -> Result<ColorReproduceInputs, &'static str> {
    let Some(ci1) = context.calibration_illuminant1_code else {
        return Err("missing calibration illuminant");
    };
    let neutral = crate::vfe::white_balance::neutral_from_metadata(
        context.as_shot_neutral,
        context.as_shot_white_xy,
        context.color_matrix2.unwrap_or(context.color_matrix1),
        context.analog_balance,
    )
    .map_err(|_| "missing AsShotNeutral and AsShotWhiteXY")?;
    Ok(ColorReproduceInputs {
        color_matrix1: matrix3(context.color_matrix1),
        color_matrix2: context.color_matrix2.map(matrix3),
        camera_calibration1: context.camera_calibration1.map(matrix3),
        camera_calibration2: context.camera_calibration2.map(matrix3),
        signatures_match: context
            .camera_calibration_signature
            .as_ref()
            .is_some_and(|camera| Some(camera) == context.profile_calibration_signature.as_ref()),
        analog_balance: context.analog_balance,
        as_shot_neutral: neutral,
        calibration_illuminant1: ci1,
        calibration_illuminant2: context.calibration_illuminant2_code,
    })
}

fn map_solver_error(module_id: &'static str) -> impl Fn(ColorReproduceError) -> OperatorError {
    move |error| OperatorError::Preprocess {
        module_id,
        reason: leak_reason(error),
    }
}

// Module ids are 'static; the solver error text is converted into a stable
// static message per variant.
fn leak_reason(error: ColorReproduceError) -> &'static str {
    match error {
        ColorReproduceError::UnsupportedIlluminant(_) => "unsupported calibration illuminant code",
        ColorReproduceError::PartialDualMetadata => "partial dual-illuminant metadata",
        ColorReproduceError::InvalidMatrix => "invalid color matrix",
        ColorReproduceError::InvalidNeutral => "invalid AsShotNeutral components",
        ColorReproduceError::SingularMatrix => "color matrix is singular",
    }
}
