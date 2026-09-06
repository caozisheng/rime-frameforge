#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_errors_doc,
    reason = "the CPU golden model narrows bounded image statistics back to f32 samples"
)]

use thiserror::Error;

use super::pyramid::PyramidImage;

pub const GUIDED_FILTER_WGSL: &str = include_str!("guided_filter.wgsl");

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum GuidedFilterError {
    #[error("guided-filter images are incompatible")]
    IncompatibleImages,
    #[error("guided-filter epsilon must be finite and non-negative")]
    InvalidEpsilon,
}

pub fn guided_filter(
    input: &PyramidImage,
    guide: &PyramidImage,
    radius: u32,
    epsilon: f32,
) -> Result<PyramidImage, GuidedFilterError> {
    if input.extent() != guide.extent() || input.channels() != guide.channels() {
        return Err(GuidedFilterError::IncompatibleImages);
    }
    if !epsilon.is_finite() || epsilon < 0.0 {
        return Err(GuidedFilterError::InvalidEpsilon);
    }
    if radius == 0 {
        return Ok(input.clone());
    }
    let (width, height) = input.extent();
    let width = width as usize;
    let height = height as usize;
    let channels = input.channels() as usize;
    let pixels = width * height;
    let mut coefficients_a = vec![0.0_f64; input.data().len()];
    let mut coefficients_b = vec![0.0_f64; input.data().len()];

    for channel in 0..channels {
        let sat_guide = summed_area(guide.data(), width, height, channels, channel, |value| {
            value
        });
        let sat_input = summed_area(input.data(), width, height, channels, channel, |value| {
            value
        });
        let sat_guide_sq = summed_area(guide.data(), width, height, channels, channel, |value| {
            value * value
        });
        let sat_cross =
            summed_area_pair(guide.data(), input.data(), width, height, channels, channel);
        for y in 0..height {
            for x in 0..width {
                let (sum_guide, area) = window_sum(&sat_guide, width, height, x, y, radius);
                let (sum_input, _) = window_sum(&sat_input, width, height, x, y, radius);
                let (sum_guide_sq, _) = window_sum(&sat_guide_sq, width, height, x, y, radius);
                let (sum_cross, _) = window_sum(&sat_cross, width, height, x, y, radius);
                let mean_guide = sum_guide / area;
                let mean_input = sum_input / area;
                let variance = (sum_guide_sq / area - mean_guide * mean_guide).max(0.0);
                let covariance = sum_cross / area - mean_guide * mean_input;
                let denominator = variance + f64::from(epsilon);
                let a = if denominator > 0.0 {
                    covariance / denominator
                } else {
                    0.0
                };
                let index = (y * width + x) * channels + channel;
                coefficients_a[index] = a;
                coefficients_b[index] = mean_input - a * mean_guide;
            }
        }
    }

    let mut output = vec![0.0; input.data().len()];
    for channel in 0..channels {
        let sat_a = summed_area_f64(&coefficients_a, width, height, channels, channel);
        let sat_b = summed_area_f64(&coefficients_b, width, height, channels, channel);
        for y in 0..height {
            for x in 0..width {
                let (sum_a, area) = window_sum(&sat_a, width, height, x, y, radius);
                let (sum_b, _) = window_sum(&sat_b, width, height, x, y, radius);
                let index = (y * width + x) * channels + channel;
                output[index] =
                    (sum_a / area * f64::from(guide.data()[index]) + sum_b / area) as f32;
            }
        }
    }
    debug_assert_eq!(output.len(), pixels * channels);
    PyramidImage::new(width as u32, height as u32, channels as u32, output)
        .map_err(|_| GuidedFilterError::IncompatibleImages)
}

fn summed_area(
    data: &[f32],
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    transform: impl Fn(f64) -> f64,
) -> Vec<f64> {
    let mut sat = vec![0.0; (width + 1) * (height + 1)];
    for y in 0..height {
        let mut row_sum = 0.0;
        for x in 0..width {
            row_sum += transform(f64::from(data[(y * width + x) * channels + channel]));
            sat[(y + 1) * (width + 1) + x + 1] = sat[y * (width + 1) + x + 1] + row_sum;
        }
    }
    sat
}

fn summed_area_f64(
    data: &[f64],
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
) -> Vec<f64> {
    let mut sat = vec![0.0; (width + 1) * (height + 1)];
    for y in 0..height {
        let mut row_sum = 0.0;
        for x in 0..width {
            row_sum += data[(y * width + x) * channels + channel];
            sat[(y + 1) * (width + 1) + x + 1] = sat[y * (width + 1) + x + 1] + row_sum;
        }
    }
    sat
}

fn summed_area_pair(
    left: &[f32],
    right: &[f32],
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
) -> Vec<f64> {
    let mut sat = vec![0.0; (width + 1) * (height + 1)];
    for y in 0..height {
        let mut row_sum = 0.0;
        for x in 0..width {
            let index = (y * width + x) * channels + channel;
            row_sum += f64::from(left[index]) * f64::from(right[index]);
            sat[(y + 1) * (width + 1) + x + 1] = sat[y * (width + 1) + x + 1] + row_sum;
        }
    }
    sat
}

fn window_sum(
    sat: &[f64],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    radius: u32,
) -> (f64, f64) {
    let radius = radius as usize;
    let x0 = x.saturating_sub(radius);
    let y0 = y.saturating_sub(radius);
    let x1 = (x + radius + 1).min(width);
    let y1 = (y + radius + 1).min(height);
    let stride = width + 1;
    let sum = sat[y1 * stride + x1] - sat[y0 * stride + x1] - sat[y1 * stride + x0]
        + sat[y0 * stride + x0];
    (sum, ((x1 - x0) * (y1 - y0)) as f64)
}
