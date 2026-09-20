use std::sync::Arc;

use thiserror::Error;

use crate::{
    FrameIdentity, ModuleParameterPacket, OperatorError, PreprocessContext, ShaderAsset,
    operator::PreprocessFn,
};
pub const LCST_AVERAGE_GRID_WIDTH: usize = 64;
pub const LCST_AVERAGE_GRID_HEIGHT: usize = 48;
pub const LCST_AVERAGE_CHANNELS: usize = 4;
pub const LCST_AVERAGE_VALUES: usize =
    LCST_AVERAGE_GRID_WIDTH * LCST_AVERAGE_GRID_HEIGHT * LCST_AVERAGE_CHANNELS;
pub const LCST_HISTOGRAM_GRID_WIDTH: usize = 16;
pub const LCST_HISTOGRAM_GRID_HEIGHT: usize = 16;
pub const LCST_HISTOGRAM_BINS: usize = 16;
pub const LCST_HISTOGRAM_VALUES: usize =
    LCST_HISTOGRAM_GRID_WIDTH * LCST_HISTOGRAM_GRID_HEIGHT * LCST_HISTOGRAM_BINS;
pub const LCST_AVERAGE_BYTES: usize = LCST_AVERAGE_VALUES * size_of::<f32>();
pub const LCST_HISTOGRAM_BYTES: usize = LCST_HISTOGRAM_VALUES * size_of::<u32>();
pub const LCST_PAYLOAD_BYTES: usize = LCST_AVERAGE_BYTES + LCST_HISTOGRAM_BYTES;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum LcstStatisticsError {
    #[error("LCST source extent must be non-zero")]
    InvalidSourceExtent,
    #[error("LCST CFA pattern must contain one R, two G, and one B site")]
    InvalidCfaPattern,
    #[error("LCST average payload has an invalid length")]
    InvalidAverageLength,
    #[error("LCST histogram payload has an invalid length")]
    InvalidHistogramLength,
    #[error("LCST average payload contains a non-finite value")]
    NonFiniteAverage,
    #[error("LCST histogram tile total does not match its source partition")]
    InvalidHistogramTotal,
    #[error("LCST payload byte length is invalid")]
    InvalidPayloadLength,
    #[error("LCST source sample payload has an invalid length")]
    InvalidSampleLength,
    #[error("LCST source block is missing a CFA channel")]
    MissingCfaChannel,
    #[error("LCST coordinate is outside the source extent")]
    InvalidCoordinate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LcstStatisticsPacket {
    identity: FrameIdentity,
    source_extent: [u32; 2],
    cfa_pattern: [u32; 4],
    average_rggb: Arc<[f32]>,
    luma_histograms: Arc<[u32]>,
}

impl LcstStatisticsPacket {
    /// Validates and freezes one LCST statistics result.
    ///
    /// # Errors
    ///
    /// Returns a stable contract error when extent, CFA, payload shape, finite
    /// values, or per-tile histogram totals are invalid.
    pub fn new(
        identity: FrameIdentity,
        width: u32,
        height: u32,
        cfa_pattern: [u32; 4],
        average_rggb: Vec<f32>,
        luma_histograms: Vec<u32>,
    ) -> Result<Self, LcstStatisticsError> {
        if width == 0 || height == 0 {
            return Err(LcstStatisticsError::InvalidSourceExtent);
        }
        if !valid_cfa_pattern(cfa_pattern) {
            return Err(LcstStatisticsError::InvalidCfaPattern);
        }
        if average_rggb.len() != LCST_AVERAGE_VALUES {
            return Err(LcstStatisticsError::InvalidAverageLength);
        }
        if luma_histograms.len() != LCST_HISTOGRAM_VALUES {
            return Err(LcstStatisticsError::InvalidHistogramLength);
        }
        if !average_rggb.iter().all(|value| value.is_finite()) {
            return Err(LcstStatisticsError::NonFiniteAverage);
        }
        validate_histogram_totals(width, height, &luma_histograms)?;
        Ok(Self {
            identity,
            source_extent: [width, height],
            cfa_pattern,
            average_rggb: average_rggb.into(),
            luma_histograms: luma_histograms.into(),
        })
    }

    #[must_use]
    pub const fn identity(&self) -> FrameIdentity {
        self.identity
    }

    #[must_use]
    pub const fn source_extent(&self) -> [u32; 2] {
        self.source_extent
    }

    #[must_use]
    pub const fn cfa_pattern(&self) -> [u32; 4] {
        self.cfa_pattern
    }

    #[must_use]
    pub fn average_rggb(&self) -> &[f32] {
        &self.average_rggb
    }

    #[must_use]
    pub fn luma_histograms(&self) -> &[u32] {
        &self.luma_histograms
    }

    pub(crate) fn shared_luma_histograms(&self) -> Arc<[u32]> {
        Arc::clone(&self.luma_histograms)
    }
}

#[must_use]
pub const fn valid_cfa_pattern(pattern: [u32; 4]) -> bool {
    let mut red = 0;
    let mut green = 0;
    let mut blue = 0;
    let mut index = 0;
    while index < pattern.len() {
        match pattern[index] {
            0 => red += 1,
            1 => green += 1,
            2 => blue += 1,
            _ => return false,
        }
        index += 1;
    }
    red == 1 && green == 2 && blue == 1
}

fn validate_histogram_totals(
    width: u32,
    height: u32,
    histograms: &[u32],
) -> Result<(), LcstStatisticsError> {
    for tile_row in 0..LCST_HISTOGRAM_GRID_HEIGHT {
        let tile_row_u32 = u32::try_from(tile_row).expect("fixed LCST tile index fits u32");
        let y0 = tile_row_u32 * height / 16;
        let y1 = (tile_row_u32 + 1) * height / 16;
        for tile_col in 0..LCST_HISTOGRAM_GRID_WIDTH {
            let tile_col_u32 = u32::try_from(tile_col).expect("fixed LCST tile index fits u32");
            let x0 = tile_col_u32 * width / 16;
            let x1 = (tile_col_u32 + 1) * width / 16;
            let tile = tile_row * LCST_HISTOGRAM_GRID_WIDTH + tile_col;
            let start = tile * LCST_HISTOGRAM_BINS;
            let total = histograms[start..start + LCST_HISTOGRAM_BINS]
                .iter()
                .copied()
                .try_fold(0_u32, u32::checked_add)
                .ok_or(LcstStatisticsError::InvalidHistogramTotal)?;
            if total != (x1 - x0) * (y1 - y0) {
                return Err(LcstStatisticsError::InvalidHistogramTotal);
            }
        }
    }
    Ok(())
}

pub type LcstDecodeFn = fn(
    FrameIdentity,
    [u32; 2],
    [u32; 4],
    &[u8],
) -> Result<LcstStatisticsPacket, LcstStatisticsError>;

#[derive(Clone, Copy, Debug)]
pub struct LcstMethodManifest {
    pub method: &'static str,
    pub shader: ShaderAsset,
    pub preprocess: PreprocessFn,
    pub decode: LcstDecodeFn,
    pub average_dispatch: [u32; 3],
    pub histogram_dispatch: [u32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct LcstProducerDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub default_method: &'static str,
    pub methods: &'static [LcstMethodManifest],
}

pub trait LcstProducer: Sync {
    fn definition(&self) -> &'static LcstProducerDefinition;

    /// Resolves a statistics method.
    ///
    /// # Errors
    ///
    /// Returns `UnknownMethod` when the method is not registered.
    fn method(&self, method: &str) -> Result<&'static LcstMethodManifest, OperatorError> {
        self.definition()
            .methods
            .iter()
            .find(|candidate| candidate.method == method)
            .ok_or_else(|| OperatorError::UnknownMethod {
                module_id: self.definition().id,
                method: method.to_owned(),
            })
    }

    /// Freezes producer parameters before GPU submission.
    ///
    /// # Errors
    ///
    /// Returns the selected method's validation error.
    fn preprocess(
        &self,
        method: &str,
        context: &PreprocessContext,
    ) -> Result<ModuleParameterPacket, OperatorError> {
        let method = self.method(method)?;
        (method.preprocess)(context, self.definition().id, method.method)
    }
}

pub struct StaticLcstProducer {
    pub definition: &'static LcstProducerDefinition,
}

impl LcstProducer for StaticLcstProducer {
    fn definition(&self) -> &'static LcstProducerDefinition {
        self.definition
    }
}
