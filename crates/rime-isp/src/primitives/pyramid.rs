#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    reason = "the CPU golden model converts validated image extents and floating sample coordinates"
)]

use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct PyramidImage {
    width: u32,
    height: u32,
    channels: u32,
    data: Vec<f32>,
}

impl PyramidImage {
    pub fn new(
        width: u32,
        height: u32,
        channels: u32,
        data: Vec<f32>,
    ) -> Result<Self, PyramidError> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| {
                usize::try_from(channels)
                    .ok()
                    .and_then(|channels| pixels.checked_mul(channels))
            })
            .ok_or(PyramidError::InvalidImage)?;
        if width == 0 || height == 0 || channels == 0 || data.len() != expected {
            return Err(PyramidError::InvalidImage);
        }
        Ok(Self {
            width,
            height,
            channels,
            data,
        })
    }

    #[must_use]
    pub const fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub const fn channels(&self) -> u32 {
        self.channels
    }

    #[must_use]
    pub fn data(&self) -> &[f32] {
        &self.data
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PyramidError {
    #[error("invalid pyramid image")]
    InvalidImage,
    #[error("pyramid level count must be positive")]
    ZeroLevels,
    #[error("pyramid levels are incompatible")]
    IncompatibleLevels,
}

#[must_use]
pub fn level_extents(width: u32, height: u32, level_count: u32) -> Vec<(u32, u32)> {
    let mut width = width.max(1);
    let mut height = height.max(1);
    let mut extents = Vec::with_capacity(level_count as usize);
    for _ in 0..level_count {
        extents.push((width, height));
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    extents
}

pub fn pyr_dec_gaussian(
    image: &PyramidImage,
    level_count: u32,
) -> Result<Vec<PyramidImage>, PyramidError> {
    if level_count == 0 {
        return Err(PyramidError::ZeroLevels);
    }
    let extents = level_extents(image.width, image.height, level_count);
    let mut levels = Vec::with_capacity(level_count as usize);
    levels.push(image.clone());
    for &(width, height) in &extents[1..] {
        let next = pyr_downscale(levels.last().expect("first level exists"), width, height)?;
        levels.push(next);
    }
    Ok(levels)
}

pub fn pyr_dec_laplacian(
    image: &PyramidImage,
    level_count: u32,
) -> Result<Vec<PyramidImage>, PyramidError> {
    let gaussian = pyr_dec_gaussian(image, level_count)?;
    let mut laplacian = Vec::with_capacity(gaussian.len());
    for index in 0..gaussian.len().saturating_sub(1) {
        let current = &gaussian[index];
        let expanded = pyr_upscale(&gaussian[index + 1], current.width, current.height)?;
        let data = current
            .data
            .iter()
            .zip(expanded.data)
            .map(|(value, expanded)| value - expanded)
            .collect();
        laplacian.push(PyramidImage::new(
            current.width,
            current.height,
            current.channels,
            data,
        )?);
    }
    laplacian.push(gaussian.last().expect("non-empty pyramid").clone());
    Ok(laplacian)
}

pub fn pyr_rec(levels: &[PyramidImage]) -> Result<PyramidImage, PyramidError> {
    let mut current = levels.last().cloned().ok_or(PyramidError::ZeroLevels)?;
    for level in levels[..levels.len() - 1].iter().rev() {
        if level.channels != current.channels {
            return Err(PyramidError::IncompatibleLevels);
        }
        let expanded = pyr_upscale(&current, level.width, level.height)?;
        let data = level
            .data
            .iter()
            .zip(expanded.data)
            .map(|(residual, expanded)| residual + expanded)
            .collect();
        current = PyramidImage::new(level.width, level.height, level.channels, data)?;
    }
    Ok(current)
}

pub fn pyr_downscale(
    image: &PyramidImage,
    width: u32,
    height: u32,
) -> Result<PyramidImage, PyramidError> {
    resize(image, width, height, true)
}

pub fn pyr_upscale(
    image: &PyramidImage,
    width: u32,
    height: u32,
) -> Result<PyramidImage, PyramidError> {
    resize(image, width, height, false)
}

fn resize(
    image: &PyramidImage,
    width: u32,
    height: u32,
    antialias: bool,
) -> Result<PyramidImage, PyramidError> {
    if width == 0 || height == 0 {
        return Err(PyramidError::InvalidImage);
    }
    if width == image.width && height == image.height {
        return Ok(image.clone());
    }
    let channels = image.channels as usize;
    let mut horizontal = vec![0.0; width as usize * image.height as usize * channels];
    for y in 0..image.height as usize {
        for x in 0..width as usize {
            let coordinate = (x as f32 + 0.5) * image.width as f32 / width as f32 - 0.5;
            let scale = if antialias && width < image.width {
                width as f32 / image.width as f32
            } else {
                1.0
            };
            let support = 2.0 / scale;
            let start = (coordinate - support).floor() as i32;
            let end = (coordinate + support).ceil() as i32;
            for channel in 0..channels {
                let mut sum = 0.0;
                let mut weight_sum = 0.0;
                for source_x in start..=end {
                    let weight = cubic((coordinate - source_x as f32) * scale) * scale;
                    if weight == 0.0 {
                        continue;
                    }
                    let source_x = mirror(source_x, image.width as i32) as usize;
                    sum += image.data[(y * image.width as usize + source_x) * channels + channel]
                        * weight;
                    weight_sum += weight;
                }
                horizontal[(y * width as usize + x) * channels + channel] = sum / weight_sum;
            }
        }
    }

    let mut output = vec![0.0; width as usize * height as usize * channels];
    for y in 0..height as usize {
        let coordinate = (y as f32 + 0.5) * image.height as f32 / height as f32 - 0.5;
        let scale = if antialias && height < image.height {
            height as f32 / image.height as f32
        } else {
            1.0
        };
        let support = 2.0 / scale;
        let start = (coordinate - support).floor() as i32;
        let end = (coordinate + support).ceil() as i32;
        for x in 0..width as usize {
            for channel in 0..channels {
                let mut sum = 0.0;
                let mut weight_sum = 0.0;
                for source_y in start..=end {
                    let weight = cubic((coordinate - source_y as f32) * scale) * scale;
                    if weight == 0.0 {
                        continue;
                    }
                    let source_y = mirror(source_y, image.height as i32) as usize;
                    sum +=
                        horizontal[(source_y * width as usize + x) * channels + channel] * weight;
                    weight_sum += weight;
                }
                output[(y * width as usize + x) * channels + channel] = sum / weight_sum;
            }
        }
    }
    PyramidImage::new(width, height, image.channels, output)
}

fn cubic(value: f32) -> f32 {
    let value = value.abs();
    if value <= 1.0 {
        (1.5 * value - 2.5) * value * value + 1.0
    } else if value < 2.0 {
        ((-0.5 * value + 2.5) * value - 4.0) * value + 2.0
    } else {
        0.0
    }
}

fn mirror(index: i32, length: i32) -> i32 {
    if length <= 1 {
        return 0;
    }
    let period = 2 * length;
    let value = index.rem_euclid(period);
    if value < length {
        value
    } else {
        period - value - 1
    }
}
