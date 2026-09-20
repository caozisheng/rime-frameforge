#![expect(
    clippy::missing_errors_doc,
    reason = "These public constructors and validators expose the native pipeline contract and are covered by their typed error enum."
)]

mod operator_scheduler;
mod wgpu_backend;

pub use operator_scheduler::{
    OperatorPhase, OperatorPhaseEvent, execute_operator_methods, execute_operator_phases,
};
pub use wgpu_backend::{RenderFeatureFlags, WgpuReadbackError, WgpuReadbackExecutor};

use rime_isp::LcstStatisticsPacket;
use std::collections::VecDeque;

use rime_core::FramePhase;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGpuBackend {
    WgpuReadback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePipelineConfig {
    pub graph_id: String,
    pub ring_capacity: usize,
    pub backend: NativeGpuBackend,
}

impl Default for NativePipelineConfig {
    fn default() -> Self {
        Self {
            graph_id: "normal".to_owned(),
            ring_capacity: 2,
            backend: NativeGpuBackend::WgpuReadback,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFrameIdentity {
    pub frame_index: u64,
    pub run_revision: u64,
    pub method_revision: u64,
    pub gpu_generation: u64,
    pub phase: FramePhase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewSurface {
    identity: NativeFrameIdentity,
    width: u32,
    height: u32,
    pixels: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceFrameOutput {
    surface: PreviewSurface,
    statistics: LcstStatisticsPacket,
}

impl SequenceFrameOutput {
    #[must_use]
    pub const fn surface(&self) -> &PreviewSurface {
        &self.surface
    }

    #[must_use]
    pub const fn statistics(&self) -> &LcstStatisticsPacket {
        &self.statistics
    }

    pub(crate) fn new(surface: PreviewSurface, statistics: LcstStatisticsPacket) -> Self {
        Self {
            surface,
            statistics,
        }
    }

    #[must_use]
    pub fn into_surface(self) -> PreviewSurface {
        self.surface
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NativePipelineError {
    #[error("native pipeline graph `{0}` is unsupported")]
    UnsupportedGraph(String),
    #[error("native pipeline graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("native pipeline graph has no preview output")]
    MissingPreview,
    #[error("native pipeline ring capacity must be at least one")]
    InvalidRingCapacity,
    #[error("preview dimensions must be non-zero")]
    InvalidPreviewExtent,
    #[error("preview RGBA sample count does not match dimensions")]
    PreviewSampleCountMismatch,
    #[error("frame slot {slot} is not present")]
    UnknownFrameSlot { slot: usize },
    #[error("invalid frame slot transition from {from:?} to {to:?}")]
    InvalidFrameTransition {
        from: FrameSlotState,
        to: FrameSlotState,
    },
}

impl PreviewSurface {
    pub fn new(
        identity: NativeFrameIdentity,
        width: u32,
        height: u32,
        pixels: Vec<f32>,
    ) -> Result<Self, NativePipelineError> {
        if width == 0 || height == 0 {
            return Err(NativePipelineError::InvalidPreviewExtent);
        }
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4));
        if expected != Some(pixels.len()) {
            return Err(NativePipelineError::PreviewSampleCountMismatch);
        }
        Ok(Self {
            identity,
            width,
            height,
            pixels,
        })
    }

    #[must_use]
    pub fn node_id(&self) -> &'static str {
        "rgb2yuv"
    }

    #[must_use]
    pub fn port_id(&self) -> &'static str {
        "out"
    }

    #[must_use]
    pub const fn identity(&self) -> NativeFrameIdentity {
        self.identity
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[f32] {
        &self.pixels
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameSlotState {
    Empty,
    Decoding,
    Decoded,
    GpuSubmitted,
    Encoded,
    Reusable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameSlot {
    state: FrameSlotState,
}

pub struct BoundedFrameRing {
    slots: Vec<FrameSlot>,
    empty: VecDeque<usize>,
}

impl BoundedFrameRing {
    pub fn new(capacity: usize) -> Result<Self, NativePipelineError> {
        if capacity == 0 {
            return Err(NativePipelineError::InvalidRingCapacity);
        }
        let slots = vec![
            FrameSlot {
                state: FrameSlotState::Empty
            };
            capacity
        ];
        let empty = (0..capacity).collect();
        Ok(Self { slots, empty })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn claim_empty(&mut self) -> Option<usize> {
        self.empty.pop_front()
    }

    pub fn transition(
        &mut self,
        slot: usize,
        to: FrameSlotState,
    ) -> Result<(), NativePipelineError> {
        let frame = self
            .slots
            .get_mut(slot)
            .ok_or(NativePipelineError::UnknownFrameSlot { slot })?;
        if !valid_transition(frame.state, to) {
            return Err(NativePipelineError::InvalidFrameTransition {
                from: frame.state,
                to,
            });
        }
        frame.state = to;
        if to == FrameSlotState::Reusable {
            self.empty.push_back(slot);
        }
        Ok(())
    }

    #[must_use]
    pub fn state(&self, slot: usize) -> Option<FrameSlotState> {
        self.slots.get(slot).map(|frame| frame.state)
    }
}

/// A consumer that may hold one lease for an exact LCST packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcstStatisticsConsumer {
    Tintless,
    Drc,
}

impl LcstStatisticsConsumer {
    const fn bit(self) -> u8 {
        match self {
            Self::Tintless => 1,
            Self::Drc => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LcstStatisticsRingError {
    InvalidCapacity,
    RingExhausted,
    MissingExactFrame { frame_index: u64, run_revision: u64 },
    ConsumerAlreadyLeased,
    InactiveLease,
}

impl std::fmt::Display for LcstStatisticsRingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCapacity => {
                formatter.write_str("LCST statistics ring capacity must be non-zero")
            }
            Self::RingExhausted => formatter.write_str("LCST statistics ring has no reusable slot"),
            Self::MissingExactFrame {
                frame_index,
                run_revision,
            } => write!(
                formatter,
                "LCST statistics frame {frame_index} with run revision {run_revision} is unavailable"
            ),
            Self::ConsumerAlreadyLeased => {
                formatter.write_str("LCST statistics consumer already has a lease")
            }
            Self::InactiveLease => formatter.write_str("LCST statistics lease is inactive"),
        }
    }
}

impl std::error::Error for LcstStatisticsRingError {}

#[derive(Clone, Debug)]
struct LcstStatisticsSlot {
    packet: Option<LcstStatisticsPacket>,
    leased_consumers: u8,
}

/// Fixed-capacity temporal storage for exact-frame LCST packets.
pub struct LcstStatisticsRing {
    slots: Vec<LcstStatisticsSlot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LcstStatisticsLease {
    packet: LcstStatisticsPacket,
    slot: usize,
    consumer: LcstStatisticsConsumer,
}

impl LcstStatisticsLease {
    #[must_use]
    pub fn packet(&self) -> &LcstStatisticsPacket {
        &self.packet
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum SequenceStatistics {
    ColdStart,
    Packet(LcstStatisticsLease),
}

impl LcstStatisticsRing {
    pub fn new(capacity: usize) -> Result<Self, LcstStatisticsRingError> {
        if capacity == 0 {
            return Err(LcstStatisticsRingError::InvalidCapacity);
        }
        Ok(Self {
            slots: (0..capacity)
                .map(|_| LcstStatisticsSlot {
                    packet: None,
                    leased_consumers: 0,
                })
                .collect(),
        })
    }

    pub fn publish(&mut self, packet: LcstStatisticsPacket) -> Result<(), LcstStatisticsRingError> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.packet.is_none())
            .ok_or(LcstStatisticsRingError::RingExhausted)?;
        slot.packet = Some(packet);
        slot.leased_consumers = 0;
        Ok(())
    }

    pub fn acquire_previous(
        &mut self,
        consumer_frame_index: u64,
        run_revision: u64,
        consumer: LcstStatisticsConsumer,
    ) -> Result<SequenceStatistics, LcstStatisticsRingError> {
        if consumer_frame_index == 0 {
            return Ok(SequenceStatistics::ColdStart);
        }
        let frame_index = consumer_frame_index - 1;
        let slot_index = self
            .slots
            .iter()
            .position(|slot| {
                slot.packet.as_ref().is_some_and(|packet| {
                    let identity = packet.identity();
                    identity.frame_index == frame_index && identity.run_revision == run_revision
                })
            })
            .ok_or(LcstStatisticsRingError::MissingExactFrame {
                frame_index,
                run_revision,
            })?;
        let slot = &mut self.slots[slot_index];
        let packet = slot
            .packet
            .as_ref()
            .ok_or(LcstStatisticsRingError::MissingExactFrame {
                frame_index,
                run_revision,
            })?
            .clone();
        let bit = consumer.bit();
        if slot.leased_consumers & bit != 0 {
            return Err(LcstStatisticsRingError::ConsumerAlreadyLeased);
        }
        slot.leased_consumers |= bit;
        Ok(SequenceStatistics::Packet(LcstStatisticsLease {
            packet,
            slot: slot_index,
            consumer,
        }))
    }

    pub fn release(&mut self, lease: &LcstStatisticsLease) -> Result<(), LcstStatisticsRingError> {
        let slot = self
            .slots
            .get_mut(lease.slot)
            .ok_or(LcstStatisticsRingError::InactiveLease)?;
        let Some(packet) = slot.packet.as_ref() else {
            return Err(LcstStatisticsRingError::InactiveLease);
        };
        if packet.identity() != lease.packet.identity() {
            return Err(LcstStatisticsRingError::InactiveLease);
        }
        let bit = lease.consumer.bit();
        if slot.leased_consumers & bit == 0 {
            return Err(LcstStatisticsRingError::InactiveLease);
        }
        slot.leased_consumers &= !bit;
        if slot.leased_consumers == 0 {
            slot.packet = None;
        }
        Ok(())
    }
}

fn valid_transition(from: FrameSlotState, to: FrameSlotState) -> bool {
    matches!(
        (from, to),
        (
            FrameSlotState::Empty | FrameSlotState::Reusable,
            FrameSlotState::Decoding
        ) | (FrameSlotState::Decoding, FrameSlotState::Decoded)
            | (FrameSlotState::Decoded, FrameSlotState::GpuSubmitted)
            | (FrameSlotState::GpuSubmitted, FrameSlotState::Encoded)
            | (FrameSlotState::Encoded, FrameSlotState::Reusable)
    )
}

pub fn validate_config(config: &NativePipelineConfig) -> Result<(), NativePipelineError> {
    if config.graph_id != "normal" {
        return Err(NativePipelineError::UnsupportedGraph(
            config.graph_id.clone(),
        ));
    }
    if config.ring_capacity == 0 {
        return Err(NativePipelineError::InvalidRingCapacity);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct NativeGraphPlan {
    manifest: rime_core::PipelineManifest,
    execution_order: Vec<String>,
    preview_node_id: String,
    preview_port_id: String,
}

impl NativeGraphPlan {
    #[must_use]
    pub fn graph_id(&self) -> &str {
        &self.manifest.graph_id
    }

    #[must_use]
    pub fn manifest_hash(&self) -> &str {
        &self.manifest.manifest_hash
    }

    #[must_use]
    pub fn execution_order(&self) -> &[String] {
        &self.execution_order
    }

    #[must_use]
    pub fn preview_node_id(&self) -> &str {
        &self.preview_node_id
    }

    #[must_use]
    pub fn preview_port_id(&self) -> &str {
        &self.preview_port_id
    }

    #[must_use]
    pub fn manifest(&self) -> &rime_core::PipelineManifest {
        &self.manifest
    }
}

pub fn build_normal_graph_plan() -> Result<NativeGraphPlan, NativePipelineError> {
    let manifest = rime_isp::build_normal_manifest();
    manifest
        .validate()
        .map_err(|error| NativePipelineError::InvalidGraph(error.to_string()))?;
    let preview = manifest
        .preview_outputs
        .first()
        .ok_or(NativePipelineError::MissingPreview)?;
    let execution_order = manifest
        .topological_order()
        .map_err(|error| NativePipelineError::InvalidGraph(error.to_string()))?
        .into_iter()
        .map(str::to_owned)
        .collect();
    Ok(NativeGraphPlan {
        preview_node_id: preview.node_id.clone(),
        preview_port_id: preview.port_id.clone(),
        manifest,
        execution_order,
    })
}

#[must_use]
pub const fn aligned_readback_bytes_per_row(width: u32) -> u32 {
    let unaligned = width.saturating_mul(16);
    unaligned.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}
