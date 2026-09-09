//! Color reproduce solver: the CPU-side dual-illuminant interpolation and
//! matrix synthesis from `color_reproduce_dng.m`. All computations happen in
//! preprocess; the shader only consumes frozen matrices and the HSV LUT.
//!
//! Lint exemptions: the color matrices carry bit-exact reference values from
//! `DataTable.mat` (golden tests assert them at 1e-6) and the f64->f32
//! narrowing is the GPU parameter contract.
#![expect(
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "reference constants use compact literals; table indices narrow to f64 mired"
)]

use std::fmt;

use super::planckian_locus::PLANCKIAN_LOCUS;

/// Illuminant chromaticity lookup, mirroring `GetIlluminantCoord.m`.
/// Codes outside the reference table are a caller-visible error.
const ILLUMINANT_XY: &[(u16, f64, f64)] = &[
    (17, 0.44758, 0.40745), // Standard light A (also ISO studio tungsten 24)
    (18, 0.3484, 0.3516),   // Standard light B
    (19, 0.3101, 0.3162),   // Standard light C
    (20, 0.33243, 0.34744), // D55
    (21, 0.31272, 0.32903), // D65
    (22, 0.29903, 0.31488), // D75
    (23, 0.34567, 0.35851), // D50
];

const MBFD: [[f64; 3]; 3] = [
    [0.7328, 0.4296, -0.1624],
    [-0.7036, 1.6975, 0.0061],
    [0.0030, 0.0136, 0.9834],
];

const D50: [f64; 3] = [0.9642, 1.0, 0.8249];
const D65: [f64; 3] = [0.9504, 1.0, 1.0889];

pub type Matrix3 = [[f64; 3]; 3];

const XYZ2RGB_PROPHOTO: Matrix3 = [
    [
        1.3457989731028284,
        -0.2555801000799755,
        -0.051106285067534014,
    ],
    [
        -0.5446224939028348,
        1.5082327413132786,
        0.020536032391479733,
    ],
    [0.0, 0.0, 1.2119675456389454],
];

const RGB2XYZ_PROPHOTO: Matrix3 = [
    [0.7977604896723025, 0.13518583717574031, 0.0313493495815248],
    [
        0.28807112822929337,
        0.7118432178101013,
        8.565396060525902e-05,
    ],
    [0.0, 0.0, 0.8251046025104601],
];

const XYZ2RGB_SRGB: Matrix3 = [
    [3.240969941904521, -1.5373831775700932, -0.4986107602930033],
    [-0.9692436362808798, 1.8759675015077206, 0.04155505740717562],
    [
        0.055630079696993635,
        -0.20397695888897652,
        1.0569715142428784,
    ],
];
#[derive(Clone, Debug)]
pub struct ColorReproduceInputs {
    pub color_matrix1: Matrix3,
    pub color_matrix2: Option<Matrix3>,
    pub camera_calibration1: Option<Matrix3>,
    pub camera_calibration2: Option<Matrix3>,
    pub signatures_match: bool,
    pub analog_balance: Option<[f64; 3]>,
    pub as_shot_neutral: [f64; 3],
    pub calibration_illuminant1: u16,
    pub calibration_illuminant2: Option<u16>,
}

#[derive(Clone, Copy, Debug)]
pub struct ColorReproduceSolution {
    pub weight1: f64,
    pub weight2: f64,
    pub xyz_neutral: [f64; 3],
    pub camera_to_xyz_d50: Matrix3,
    pub sensor_to_prophoto: Matrix3,
    pub prophoto_to_srgb: Matrix3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorReproduceError {
    UnsupportedIlluminant(u16),
    PartialDualMetadata,
    InvalidMatrix,
    InvalidNeutral,
    SingularMatrix,
}

impl fmt::Display for ColorReproduceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedIlluminant(code) => {
                write!(f, "unsupported calibration illuminant code {code}")
            }
            Self::PartialDualMetadata => write!(f, "partial dual-illuminant metadata"),
            Self::InvalidMatrix => write!(f, "invalid color matrix"),
            Self::InvalidNeutral => write!(f, "invalid AsShotNeutral components"),
            Self::SingularMatrix => write!(f, "color matrix is singular"),
        }
    }
}

impl std::error::Error for ColorReproduceError {}

/// Solves the full color-reproduce chain for one frame's metadata.
///
/// # Errors
///
/// Returns a stable error when illuminant codes are unsupported or ordering
/// is invalid, matrices are non-finite/singular, or the dual-illuminant
struct IlluminantPairs {
    warm_mired: f64,
    cold_mired: f64,
    warm_matrix: Matrix3,
    cold_matrix: Matrix3,
    warm_calibration: Matrix3,
    cold_calibration: Matrix3,
    swapped: bool,
}

/// Resolves which matrix pairs with which illuminant, reordered warm-first.
///
/// # Errors
///
/// Returns [`ColorReproduceError::UnsupportedIlluminant`] when an illuminant
/// code is outside the reference table.
fn resolve_illuminant_pairs(
    cm1: Matrix3,
    cm2: Matrix3,
    cc1: Matrix3,
    cc2: Matrix3,
    dual: Option<(Matrix3, u16)>,
    ci1: u16,
) -> Result<IlluminantPairs, ColorReproduceError> {
    let Some((_, ci2)) = dual else {
        let mired = illuminant_mired(ci1)?;
        return Ok(IlluminantPairs {
            warm_mired: mired,
            cold_mired: mired,
            warm_matrix: cm1,
            cold_matrix: cm2,
            warm_calibration: cc1,
            cold_calibration: cc2,
            swapped: false,
        });
    };
    let mired1 = illuminant_mired(ci1)?;
    let mired2 = illuminant_mired(ci2)?;
    let (
        warm_mired,
        cold_mired,
        warm_matrix,
        cold_matrix,
        warm_calibration,
        cold_calibration,
        swapped,
    ) = if mired1 < mired2 {
        (mired2, mired1, cm2, cm1, cc2, cc1, true)
    } else {
        (mired1, mired2, cm1, cm2, cc1, cc2, false)
    };
    Ok(IlluminantPairs {
        warm_mired,
        cold_mired,
        warm_matrix,
        cold_matrix,
        warm_calibration,
        cold_calibration,
        swapped,
    })
}
/// Solves the full color-reproduce chain for one frame's metadata.
///
/// # Errors
///
/// Returns a stable error when illuminant codes are unsupported, matrices
/// are non-finite/singular, or the dual-illuminant metadata is partial.
pub fn solve_color_reproduce(
    inputs: &ColorReproduceInputs,
) -> Result<ColorReproduceSolution, ColorReproduceError> {
    let dual = match (inputs.color_matrix2, inputs.calibration_illuminant2) {
        (Some(matrix), Some(code)) => Some((matrix, code)),
        (None, None) => None,
        _ => return Err(ColorReproduceError::PartialDualMetadata),
    };
    let cm2 = dual.map_or(identity(), |(matrix, _)| matrix);
    let (cc1, cc2) = if inputs.signatures_match {
        (
            inputs.camera_calibration1.unwrap_or_else(identity),
            inputs.camera_calibration2.unwrap_or_else(identity),
        )
    } else {
        (identity(), identity())
    };
    let cm1 = inputs.color_matrix1;
    let neutral = inputs.as_shot_neutral;
    if !neutral
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(ColorReproduceError::InvalidNeutral);
    }
    for matrix in [cm1, cm2, cc1, cc2] {
        if !matrix.iter().flatten().all(|value| value.is_finite()) {
            return Err(ColorReproduceError::InvalidMatrix);
        }
    }
    let ab = match inputs.analog_balance {
        Some([r, g, b]) if [r, g, b].iter().all(|v| v.is_finite() && *v > 0.0) => {
            [[r, 0.0, 0.0], [0.0, g, 0.0], [0.0, 0.0, b]]
        }
        Some(_) => return Err(ColorReproduceError::InvalidMatrix),
        None => identity(),
    };

    // Some exporters (DJI X5S) write the illuminant slots cold-first. Each
    // matrix stays paired with its own illuminant; the pairs are reordered
    // warm-first so the mired interpolation is well-defined.
    let pairs = resolve_illuminant_pairs(cm1, cm2, cc1, cc2, dual, inputs.calibration_illuminant1)?;
    let degenerate = dual.is_none_or(|(_, ci2)| ci2 == inputs.calibration_illuminant1);
    let mut xyz = chrom(D50);
    let mut weights = if degenerate {
        (1.0, 0.0)
    } else {
        weight_factor(pairs.warm_mired, pairs.cold_mired, &xyz)
    };
    let mut xyz_neutral = xyz;
    let mut camera_to_xyz = [[0.0; 3]; 3];
    for _ in 0..100 {
        if !degenerate {
            weights = weight_factor(pairs.warm_mired, pairs.cold_mired, &xyz);
        }
        let xyz_to_camera = mul(
            ab,
            mul(
                add(
                    scale(pairs.warm_calibration, weights.0),
                    scale(pairs.cold_calibration, weights.1),
                ),
                add(
                    scale(pairs.warm_matrix, weights.0),
                    scale(pairs.cold_matrix, weights.1),
                ),
            ),
        );
        let Some(inverse) = invert(&xyz_to_camera) else {
            return Err(ColorReproduceError::SingularMatrix);
        };
        camera_to_xyz = inverse;
        let neutral_xyz = mul_vec(camera_to_xyz, neutral);
        xyz_neutral = chrom(neutral_xyz);
        let distance =
            ((xyz[0] - xyz_neutral[0]).powi(2) + (xyz[1] - xyz_neutral[1]).powi(2)).sqrt();
        if distance < 1.0e-8 {
            break;
        }
        xyz = xyz_neutral;
    }

    // Bradford adaptation from the solved neutral white to D50, then the
    // frozen sensor -> ProPhoto matrix with the un-normalized neutral folded
    // in (the reverse white balance of MATLAB step 4).
    let adaptation = bradford(neutral_xyz_unnormalized(camera_to_xyz, neutral), D50);
    let camera_to_xyz_d50 = mul(adaptation, camera_to_xyz);
    let sensor_to_prophoto = mul(XYZ2RGB_PROPHOTO, mul(camera_to_xyz_d50, diag(neutral)));
    let prophoto_to_srgb = mul(XYZ2RGB_SRGB, mul(bradford(D50, D65), RGB2XYZ_PROPHOTO));
    if ![sensor_to_prophoto, prophoto_to_srgb]
        .iter()
        .flatten()
        .flatten()
        .all(|value| value.is_finite())
    {
        return Err(ColorReproduceError::SingularMatrix);
    }
    Ok(assemble_solution(
        weights,
        pairs.swapped,
        xyz_neutral,
        camera_to_xyz_d50,
        sensor_to_prophoto,
        prophoto_to_srgb,
    ))
}

fn assemble_solution(
    weights: (f64, f64),
    swapped: bool,
    xyz_neutral: [f64; 3],
    camera_to_xyz_d50: Matrix3,
    sensor_to_prophoto: Matrix3,
    prophoto_to_srgb: Matrix3,
) -> ColorReproduceSolution {
    // Weights are reported in the original slot order (weight1 pairs
    // color_matrix1).
    let (matrix1_share, matrix2_share) = if swapped {
        (weights.1, weights.0)
    } else {
        (weights.0, weights.1)
    };
    ColorReproduceSolution {
        weight1: matrix1_share,
        weight2: matrix2_share,
        xyz_neutral,
        camera_to_xyz_d50,
        sensor_to_prophoto,
        prophoto_to_srgb,
    }
}

fn neutral_xyz_unnormalized(camera_to_xyz: Matrix3, neutral: [f64; 3]) -> [f64; 3] {
    mul_vec(camera_to_xyz, neutral)
}

const fn identity() -> Matrix3 {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

fn chrom(xyz: [f64; 3]) -> [f64; 3] {
    let sum = xyz[0] + xyz[1] + xyz[2];
    if xyz.iter().all(|value| value.abs() <= 1.0e-10) || sum == 0.0 {
        return [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
    }
    [xyz[0] / sum, xyz[1] / sum, xyz[2] / sum]
}

fn xy_to_uv(x: f64, y: f64) -> (f64, f64) {
    let denominator = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / denominator, 6.0 * y / denominator)
}

/// CCT lookup along the tabulated Planckian locus, mirroring
/// `GetColorTemperature.m`: find the tangent-sign flip interval, then
/// inverse-distance interpolate the mired value.
fn mired_from_uv(u: f64, v: f64) -> f64 {
    let signed = |entry: (f32, f32, f32)| {
        let (pu, pv, slope) = (f64::from(entry.0), f64::from(entry.1), f64::from(entry.2));
        ((v - pv) - slope * (u - pu)) / (1.0 + slope * slope).sqrt()
    };
    let mut index = 0;
    for candidate in 0..PLANCKIAN_LOCUS.len() - 1 {
        if signed(PLANCKIAN_LOCUS[candidate]).signum()
            != signed(PLANCKIAN_LOCUS[candidate + 1]).signum()
        {
            index = candidate;
            break;
        }
    }
    let (u1, v1, s1) = PLANCKIAN_LOCUS[index];
    let (u2, v2, s2) = PLANCKIAN_LOCUS[index + 1];
    let (u1, v1, s1) = (f64::from(u1), f64::from(v1), f64::from(s1));
    let (u2, v2, s2) = (f64::from(u2), f64::from(v2), f64::from(s2));
    let d1 = (((v1 - v) - s1 * (u1 - u)) / (1.0 + s1 * s1).sqrt()).abs();
    let d2 = (((v2 - v) - s2 * (u2 - u)) / (1.0 + s2 * s2).sqrt()).abs();
    let m1 = 20.0 + index as f64;
    let m2 = 21.0 + index as f64;
    m2 * d1 / (d1 + d2) + m1 * d2 / (d1 + d2)
}

fn illuminant_xy(code: u16) -> Result<(f64, f64), ColorReproduceError> {
    ILLUMINANT_XY
        .iter()
        .find(|(candidate, _, _)| *candidate == code)
        .map(|&(_, x, y)| (x, y))
        .ok_or(ColorReproduceError::UnsupportedIlluminant(code))
}

fn illuminant_mired(code: u16) -> Result<f64, ColorReproduceError> {
    let (x, y) = illuminant_xy(code)?;
    let (u, v) = xy_to_uv(x, y);
    Ok(mired_from_uv(u, v))
}

fn weight_factor(mired1: f64, mired2: f64, xyz: &[f64; 3]) -> (f64, f64) {
    let (u, v) = xy_to_uv(xyz[0], xyz[1]);
    let mired = mired_from_uv(u, v);
    if mired >= mired1 {
        (1.0, 0.0)
    } else if mired <= mired2 {
        (0.0, 1.0)
    } else {
        (
            (mired - mired2) / (mired1 - mired2),
            (mired1 - mired) / (mired1 - mired2),
        )
    }
}

fn bradford(source_white: [f64; 3], target_white: [f64; 3]) -> Matrix3 {
    let src = mul_vec(MBFD, source_white);
    let dst = mul_vec(MBFD, target_white);
    let scale = diag([dst[0] / src[0], dst[1] / src[1], dst[2] / src[2]]);
    mul(
        invert(&MBFD).expect("Bradford matrix is invertible"),
        mul(scale, MBFD),
    )
}

const fn diag(v: [f64; 3]) -> Matrix3 {
    [[v[0], 0.0, 0.0], [0.0, v[1], 0.0], [0.0, 0.0, v[2]]]
}

const fn add(a: Matrix3, b: Matrix3) -> Matrix3 {
    let mut out = [[0.0; 3]; 3];
    let mut row = 0;
    while row < 3 {
        let mut col = 0;
        while col < 3 {
            out[row][col] = a[row][col] + b[row][col];
            col += 1;
        }
        row += 1;
    }
    out
}

const fn scale(a: Matrix3, factor: f64) -> Matrix3 {
    let mut out = [[0.0; 3]; 3];
    let mut row = 0;
    while row < 3 {
        let mut col = 0;
        while col < 3 {
            out[row][col] = a[row][col] * factor;
            col += 1;
        }
        row += 1;
    }
    out
}

const fn mul(a: Matrix3, b: Matrix3) -> Matrix3 {
    let mut out = [[0.0; 3]; 3];
    let mut row = 0;
    while row < 3 {
        let mut col = 0;
        while col < 3 {
            out[row][col] = a[row][0] * b[0][col] + a[row][1] * b[1][col] + a[row][2] * b[2][col];
            col += 1;
        }
        row += 1;
    }
    out
}

const fn mul_vec(a: Matrix3, v: [f64; 3]) -> [f64; 3] {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

fn invert(matrix: &Matrix3) -> Option<Matrix3> {
    let [a, b, c, d, e, f, g, h, i] = [
        matrix[0][0],
        matrix[0][1],
        matrix[0][2],
        matrix[1][0],
        matrix[1][1],
        matrix[1][2],
        matrix[2][0],
        matrix[2][1],
        matrix[2][2],
    ];
    let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }
    let mut inverse = [
        (e * i - f * h) / determinant,
        (c * h - b * i) / determinant,
        (b * f - c * e) / determinant,
        (f * g - d * i) / determinant,
        (a * i - c * g) / determinant,
        (c * d - a * f) / determinant,
        (d * h - e * g) / determinant,
        (b * g - a * h) / determinant,
        (a * e - b * d) / determinant,
    ];
    if !inverse.iter().all(|value| value.is_finite()) {
        return None;
    }
    for value in &mut inverse {
        if value.is_nan() {
            return None;
        }
    }
    Some([
        [inverse[0], inverse[1], inverse[2]],
        [inverse[3], inverse[4], inverse[5]],
        [inverse[6], inverse[7], inverse[8]],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(
        clippy::excessive_precision,
        reason = "f32 round-trip digits of the generated table"
    )]
    fn planckian_table_extents_match_reference() {
        assert_eq!(PLANCKIAN_LOCUS.len(), 481);
        // First row: mired 20 (50000 K); last row: mired 500 (2000 K).
        // Tolerances reflect the f32 storage of the generated table.
        assert!((PLANCKIAN_LOCUS[0].0 - 0.181_325_28).abs() < 1e-7);
        assert!((PLANCKIAN_LOCUS[480].0 - 0.305_048_62).abs() < 1e-7);
        assert!((PLANCKIAN_LOCUS[0].1 - 0.268_455_39).abs() < 1e-7);
        assert!((PLANCKIAN_LOCUS[480].1 - 0.359_065_79).abs() < 1e-7);
    }

    #[test]
    fn std_a_and_d65_mired_match_reference() {
        let mired_a = illuminant_mired(17).expect("StdA in table");
        let mired_d65 = illuminant_mired(21).expect("D65 in table");
        // f32 table storage moves the interpolated mired by ~1e-4.
        assert!((mired_a - 350.197_805_292_880_84).abs() < 1e-3, "{mired_a}");
        assert!(
            (mired_d65 - 153.774_412_493_575_82).abs() < 1e-3,
            "{mired_d65}"
        );
    }
}
