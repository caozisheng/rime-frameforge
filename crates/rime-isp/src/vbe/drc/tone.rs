#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    reason = "tone LUT indices and normalized histogram ratios are bounded by validated resource extents"
)]

use thiserror::Error;

const BEZIER_SAMPLES: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct ToneLut {
    values: Vec<f32>,
}

impl ToneLut {
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    #[must_use]
    pub fn sample(&self, input: f32) -> f32 {
        if input <= 0.0 {
            return self.values[0];
        }
        if input >= 1.0 {
            return *self.values.last().expect("tone LUT is non-empty");
        }
        let position = input * (self.values.len() - 1) as f32;
        let lower = position.floor() as usize;
        let fraction = position - lower as f32;
        self.values[lower] * (1.0 - fraction) + self.values[lower + 1] * fraction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrcLocalStatistics {
    tiles_x: u32,
    tiles_y: u32,
    bins: u32,
    histograms: Vec<u32>,
}

impl DrcLocalStatistics {
    pub fn new(
        tiles_x: u32,
        tiles_y: u32,
        bins: u32,
        histograms: Vec<u32>,
    ) -> Result<Self, DrcToneError> {
        let expected = usize::try_from(tiles_x)
            .ok()
            .and_then(|x| usize::try_from(tiles_y).ok().and_then(|y| x.checked_mul(y)))
            .and_then(|tiles| {
                usize::try_from(bins)
                    .ok()
                    .and_then(|bins| tiles.checked_mul(bins))
            })
            .ok_or(DrcToneError::InvalidStatistics)?;
        if tiles_x == 0 || tiles_y == 0 || bins < 2 || histograms.len() != expected {
            return Err(DrcToneError::InvalidStatistics);
        }
        Ok(Self {
            tiles_x,
            tiles_y,
            bins,
            histograms,
        })
    }

    #[must_use]
    pub const fn tiles_x(&self) -> u32 {
        self.tiles_x
    }

    #[must_use]
    pub const fn tiles_y(&self) -> u32 {
        self.tiles_y
    }

    #[must_use]
    pub const fn bins(&self) -> u32 {
        self.bins
    }

    fn tile_histogram(&self, tile: usize) -> &[u32] {
        let bins = self.bins as usize;
        &self.histograms[tile * bins..(tile + 1) * bins]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalToneConfig {
    pub local_strength: f32,
    pub spatial_smoothing_passes: u32,
    pub minimum_samples: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalToneLutField {
    tiles_x: u32,
    tiles_y: u32,
    samples: usize,
    values: Vec<f32>,
}

impl LocalToneLutField {
    #[must_use]
    pub const fn tiles_x(&self) -> u32 {
        self.tiles_x
    }

    #[must_use]
    pub const fn tiles_y(&self) -> u32 {
        self.tiles_y
    }

    #[must_use]
    pub fn tile_values(&self, tile: usize) -> &[f32] {
        &self.values[tile * self.samples..(tile + 1) * self.samples]
    }

    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DrcToneError {
    #[error("invalid DRC tone parameter")]
    InvalidParameter,
    #[error("invalid DRC local statistics")]
    InvalidStatistics,
}

pub fn generate_global_tone_lut(
    drc_gain: f32,
    knee: f32,
    samples: usize,
) -> Result<ToneLut, DrcToneError> {
    if !drc_gain.is_finite()
        || !knee.is_finite()
        || drc_gain <= 0.0
        || knee <= 0.0
        || knee > 1.0
        || samples < 2
    {
        return Err(DrcToneError::InvalidParameter);
    }
    let mut curve = Vec::with_capacity(BEZIER_SAMPLES + 1);
    for index in 0..=BEZIER_SAMPLES {
        let t = index as f32 / BEZIER_SAMPLES as f32;
        let one_minus_t = 1.0 - t;
        let x = 2.0 * one_minus_t * t * knee / drc_gain + t * t;
        let y = 2.0 * one_minus_t * t * knee + t * t;
        curve.push((x, y));
    }
    let mut segment = 0;
    let values = (0..samples)
        .map(|index| {
            let x = index as f32 / (samples - 1) as f32;
            while segment + 1 < curve.len() && curve[segment + 1].0 < x {
                segment += 1;
            }
            if segment + 1 == curve.len() {
                return 1.0;
            }
            let (x0, y0) = curve[segment];
            let (x1, y1) = curve[segment + 1];
            if x1 <= x0 {
                y1
            } else {
                let fraction = (x - x0) / (x1 - x0);
                y0 * (1.0 - fraction) + y1 * fraction
            }
        })
        .collect();
    Ok(ToneLut { values })
}

pub fn generate_local_tone_lut(
    statistics: &DrcLocalStatistics,
    global: &ToneLut,
    config: LocalToneConfig,
) -> Result<LocalToneLutField, DrcToneError> {
    if !config.local_strength.is_finite()
        || !(0.0..=1.0).contains(&config.local_strength)
        || global.values.len() < 2
    {
        return Err(DrcToneError::InvalidParameter);
    }
    let tile_count = statistics.tiles_x as usize * statistics.tiles_y as usize;
    let samples = global.values.len();
    let mut valid = vec![false; tile_count];
    let mut values = vec![0.0; tile_count * samples];
    for tile in 0..tile_count {
        let histogram = statistics.tile_histogram(tile);
        let total: u64 = histogram.iter().map(|&count| u64::from(count)).sum();
        let output = &mut values[tile * samples..(tile + 1) * samples];
        let occupied_bins = histogram.iter().filter(|&&count| count != 0).count();
        if total < u64::from(config.minimum_samples) || occupied_bins <= 1 {
            output.copy_from_slice(global.values());
            continue;
        }
        valid[tile] = true;
        let bin_scale = (histogram.len() - 1) as f64;
        let mean = histogram
            .iter()
            .enumerate()
            .map(|(index, &count)| index as f64 / bin_scale * f64::from(count))
            .sum::<f64>()
            / total as f64;
        let variance = histogram
            .iter()
            .enumerate()
            .map(|(index, &count)| {
                let difference = index as f64 / bin_scale - mean;
                difference * difference * f64::from(count)
            })
            .sum::<f64>()
            / total as f64;
        let sample_confidence =
            (total as f32 / config.minimum_samples.max(1) as f32 / 4.0).clamp(0.0, 1.0);
        let spread_confidence = (variance.sqrt() as f32 * 8.0).clamp(0.0, 1.0);
        let local_weight = config.local_strength * sample_confidence * spread_confidence;
        let clipped_low = total / 100;
        let clipped_high = total.saturating_sub(clipped_low);
        let denominator = clipped_high.saturating_sub(clipped_low).max(1);
        for (index, value) in output.iter_mut().enumerate() {
            let bin = index * (histogram.len() - 1) / (samples - 1);
            let cumulative: u64 = histogram[..=bin]
                .iter()
                .map(|&count| u64::from(count))
                .sum();
            let clipped = cumulative.clamp(clipped_low, clipped_high);
            let cdf = clipped.saturating_sub(clipped_low) as f32 / denominator as f32;
            let constrained_target = global.sample(cdf.clamp(0.0, 1.0));
            *value =
                global.values[index] * (1.0 - local_weight) + constrained_target * local_weight;
        }
        output[0] = 0.0;
        output[samples - 1] = 1.0;
        for index in 1..samples {
            output[index] = output[index].max(output[index - 1]);
        }
    }

    for _ in 0..config.spatial_smoothing_passes {
        let previous = values.clone();
        for tile in 0..tile_count {
            if !valid[tile] {
                continue;
            }
            let x = tile % statistics.tiles_x as usize;
            let y = tile / statistics.tiles_x as usize;
            let output = &mut values[tile * samples..(tile + 1) * samples];
            for sample in 0..samples {
                let mut sum = 0.0;
                let mut weight_sum = 0.0;
                for ny in y.saturating_sub(1)..=(y + 1).min(statistics.tiles_y as usize - 1) {
                    for nx in x.saturating_sub(1)..=(x + 1).min(statistics.tiles_x as usize - 1) {
                        let neighbor = ny * statistics.tiles_x as usize + nx;
                        if valid[neighbor] {
                            let x_weight = if nx == x { 2.0 } else { 1.0 };
                            let y_weight = if ny == y { 2.0 } else { 1.0 };
                            let weight = x_weight * y_weight;
                            sum += previous[neighbor * samples + sample] * weight;
                            weight_sum += weight;
                        }
                    }
                }
                output[sample] = sum / weight_sum;
            }
        }
    }

    Ok(LocalToneLutField {
        tiles_x: statistics.tiles_x,
        tiles_y: statistics.tiles_y,
        samples,
        values,
    })
}
