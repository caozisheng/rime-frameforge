#![expect(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_panics_doc,
    reason = "LCST dimensions and histogram values are validated fixed-grid contracts"
)]
use crate::{
    LCST_AVERAGE_CHANNELS, LCST_AVERAGE_GRID_HEIGHT, LCST_AVERAGE_GRID_WIDTH, LCST_AVERAGE_VALUES,
    LCST_HISTOGRAM_BINS, LCST_HISTOGRAM_VALUES, LcstStatisticsError, valid_cfa_pattern,
};

pub const D50_GAINS: [f32; 4] = [2.804_687_5, 1.0, 1.0, 1.742_187_5];
const GAUSSIAN_WEIGHTS: [[f32; 3]; 3] = [[1.0, 2.0, 1.0], [2.0, 4.0, 2.0], [1.0, 2.0, 1.0]];

#[must_use]
pub const fn partition_bounds(index: u32, parts: u32, size: u32) -> [u32; 2] {
    [index * size / parts, (index + 1) * size / parts]
}

#[must_use]
pub fn block_center(index: u32, parts: u32, size: u32) -> f32 {
    let [start, end] = partition_bounds(index, parts, size);
    (start + end - 1) as f32 * 0.5
}

/// Resolves a Bayer site to the LCST `[R, Gr, Gb, B]` channel index.
///
/// # Errors
///
/// Returns `InvalidCfaPattern` unless the 2x2 period contains one red, two
/// greens, and one blue site.
pub fn cfa_channel(cfa_pattern: [u32; 4], x: u32, y: u32) -> Result<usize, LcstStatisticsError> {
    if !valid_cfa_pattern(cfa_pattern) {
        return Err(LcstStatisticsError::InvalidCfaPattern);
    }
    let site = usize::try_from((y & 1) * 2 + (x & 1)).expect("Bayer site fits usize");
    match cfa_pattern[site] {
        0 => Ok(0),
        2 => Ok(3),
        1 => {
            let red_site = cfa_pattern
                .iter()
                .position(|value| *value == 0)
                .expect("validated CFA has red");
            if site / 2 == red_site / 2 {
                Ok(1)
            } else {
                Ok(2)
            }
        }
        _ => Err(LcstStatisticsError::InvalidCfaPattern),
    }
}

/// Validates the fixed LCST grid against one source frame.
///
/// # Errors
///
/// Returns a stable contract error for invalid extent/CFA or any average block
/// that does not contain all four Bayer sites.
pub fn validate_source(
    width: u32,
    height: u32,
    cfa_pattern: [u32; 4],
) -> Result<(), LcstStatisticsError> {
    if width == 0 || height == 0 {
        return Err(LcstStatisticsError::InvalidSourceExtent);
    }
    if !valid_cfa_pattern(cfa_pattern) {
        return Err(LcstStatisticsError::InvalidCfaPattern);
    }
    for block_y in 0..u32::try_from(LCST_AVERAGE_GRID_HEIGHT).expect("fixed grid fits u32") {
        let [y0, y1] = partition_bounds(block_y, 48, height);
        for block_x in 0..u32::try_from(LCST_AVERAGE_GRID_WIDTH).expect("fixed grid fits u32") {
            let [x0, x1] = partition_bounds(block_x, 64, width);
            let mut present = [false; LCST_AVERAGE_CHANNELS];
            for y in y0..y1 {
                for x in x0..x1 {
                    present[cfa_channel(cfa_pattern, x, y)?] = true;
                }
            }
            if !present.into_iter().all(|value| value) {
                return Err(LcstStatisticsError::MissingCfaChannel);
            }
        }
    }
    Ok(())
}

/// Computes the deterministic CPU reference for LCST RGGB averages.
///
/// # Errors
///
/// Returns a stable contract error for invalid source metadata or sample count.
pub fn average_rggb_reference(
    samples: &[f32],
    width: u32,
    height: u32,
    cfa_pattern: [u32; 4],
) -> Result<Vec<f32>, LcstStatisticsError> {
    validate_samples(samples, width, height)?;
    validate_source(width, height, cfa_pattern)?;
    let mut output = vec![0.0; LCST_AVERAGE_VALUES];
    for block_y in 0..48_u32 {
        let [y0, y1] = partition_bounds(block_y, 48, height);
        for block_x in 0..64_u32 {
            let [x0, x1] = partition_bounds(block_x, 64, width);
            let mut sums = [0.0_f32; 4];
            let mut counts = [0_u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let channel = cfa_channel(cfa_pattern, x, y)?;
                    sums[channel] += samples[sample_index(x, y, width)];
                    counts[channel] += 1;
                }
            }
            let base = (usize::try_from(block_y).expect("grid y fits usize")
                * LCST_AVERAGE_GRID_WIDTH
                + usize::try_from(block_x).expect("grid x fits usize"))
                * LCST_AVERAGE_CHANNELS;
            for channel in 0..LCST_AVERAGE_CHANNELS {
                output[base + channel] = sums[channel] / counts[channel] as f32;
            }
        }
    }
    Ok(output)
}

/// Computes white-balanced, Gaussian-filtered Bayer intensity at one pixel.
///
/// # Errors
///
/// Returns a stable contract error for invalid metadata, sample count, or coordinate.
pub fn filtered_luma_at(
    samples: &[f32],
    width: u32,
    height: u32,
    cfa_pattern: [u32; 4],
    x: u32,
    y: u32,
) -> Result<f32, LcstStatisticsError> {
    validate_samples(samples, width, height)?;
    if x >= width || y >= height {
        return Err(LcstStatisticsError::InvalidCoordinate);
    }
    if !valid_cfa_pattern(cfa_pattern) {
        return Err(LcstStatisticsError::InvalidCfaPattern);
    }
    let mut sum = 0.0;
    for (kernel_y, weights) in GAUSSIAN_WEIGHTS.iter().enumerate() {
        let source_y = clamp_offset(y, kernel_y, height);
        for (kernel_x, weight) in weights.iter().enumerate() {
            let source_x = clamp_offset(x, kernel_x, width);
            let channel = cfa_channel(cfa_pattern, source_x, source_y)?;
            sum += samples[sample_index(source_x, source_y, width)] * D50_GAINS[channel] * weight;
        }
    }
    Ok(sum / 16.0)
}

/// Computes the deterministic CPU reference for LCST luma histograms.
///
/// # Errors
///
/// Returns a stable contract error for invalid source metadata or sample count.
pub fn histogram_reference(
    samples: &[f32],
    width: u32,
    height: u32,
    cfa_pattern: [u32; 4],
) -> Result<Vec<u32>, LcstStatisticsError> {
    validate_samples(samples, width, height)?;
    if !valid_cfa_pattern(cfa_pattern) {
        return Err(LcstStatisticsError::InvalidCfaPattern);
    }
    let mut output = vec![0_u32; LCST_HISTOGRAM_VALUES];
    for tile_y in 0..16_u32 {
        let [y0, y1] = partition_bounds(tile_y, 16, height);
        for tile_x in 0..16_u32 {
            let [x0, x1] = partition_bounds(tile_x, 16, width);
            let tile = usize::try_from(tile_y * 16 + tile_x).expect("tile index fits usize");
            for y in y0..y1 {
                for x in x0..x1 {
                    let value = filtered_luma_at(samples, width, height, cfa_pattern, x, y)?;
                    output[tile * LCST_HISTOGRAM_BINS + histogram_bin(value)] += 1;
                }
            }
        }
    }
    Ok(output)
}

#[must_use]
pub fn histogram_bin(value: f32) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    if value >= 1.0 {
        return LCST_HISTOGRAM_BINS - 1;
    }
    (value * LCST_HISTOGRAM_BINS as f32) as usize
}

fn validate_samples(samples: &[f32], width: u32, height: u32) -> Result<(), LcstStatisticsError> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(LcstStatisticsError::InvalidSourceExtent)?;
    if expected == 0 {
        return Err(LcstStatisticsError::InvalidSourceExtent);
    }
    if samples.len() != expected {
        return Err(LcstStatisticsError::InvalidSampleLength);
    }
    Ok(())
}

fn sample_index(x: u32, y: u32, width: u32) -> usize {
    usize::try_from(y * width + x).expect("validated source index fits usize")
}

fn clamp_offset(coordinate: u32, kernel_index: usize, extent: u32) -> u32 {
    let offset = i32::try_from(kernel_index).expect("kernel index fits i32") - 1;
    coordinate.saturating_add_signed(offset).min(extent - 1)
}
