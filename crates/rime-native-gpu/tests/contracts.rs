use rime_core::FramePhase;
use rime_native_gpu::{
    BoundedFrameRing, FrameSlotState, NativeFrameIdentity, NativeGpuBackend, NativePipelineConfig,
    PreviewSurface,
};

#[test]
fn native_config_reuses_normal_graph_and_default_two_slot_ring() {
    let config = NativePipelineConfig::default();

    assert_eq!(config.graph_id, "normal");
    assert_eq!(config.ring_capacity, 2);
    assert_eq!(config.backend, NativeGpuBackend::WgpuReadback);
}

#[test]
fn preview_surface_preserves_existing_frame_identity_contract() {
    let identity = NativeFrameIdentity {
        frame_index: 12,
        run_revision: 3,
        method_revision: 4,
        gpu_generation: 5,
        phase: FramePhase::Output,
    };
    let surface = PreviewSurface::new(identity, 2, 1, vec![0.0; 8]).expect("surface is valid");

    assert_eq!(surface.node_id(), "rgb2yuv");
    assert_eq!(surface.port_id(), "out");
    assert_eq!(surface.width(), 2);
    assert_eq!(surface.height(), 1);
    assert_eq!(surface.identity(), identity);
}

#[test]
fn frame_ring_enforces_bounded_state_transitions() {
    let mut ring = BoundedFrameRing::new(2).expect("capacity is valid");
    let first = ring.claim_empty().expect("first slot is available");
    ring.transition(first, FrameSlotState::Decoding)
        .expect("empty slot may begin decoding");
    ring.transition(first, FrameSlotState::Decoded)
        .expect("decoding slot may become decoded");
    ring.transition(first, FrameSlotState::GpuSubmitted)
        .expect("decoded slot may submit to GPU");
    ring.transition(first, FrameSlotState::Encoded)
        .expect("submitted slot may become encoded");
    ring.transition(first, FrameSlotState::Reusable)
        .expect("encoded slot may return to reusable state");

    assert_eq!(ring.len(), 2);
    assert_eq!(ring.state(first), Some(FrameSlotState::Reusable));
    assert!(ring.claim_empty().is_some());
}

#[test]
fn native_graph_plan_matches_v013_normal_manifest() {
    let plan = rime_native_gpu::build_normal_graph_plan().expect("normal graph must be valid");

    assert_eq!(plan.graph_id(), "normal");
    assert_eq!(plan.preview_node_id(), "rgb2yuv");
    assert_eq!(plan.preview_port_id(), "out");
    assert_eq!(
        plan.execution_order().first().map(String::as_str),
        Some("raw_source")
    );
    assert_eq!(
        plan.execution_order().last().map(String::as_str),
        Some("rgb2yuv")
    );
    assert_eq!(
        plan.manifest_hash(),
        "cb74b6eee78950045516df402a9be2086820433d0ab2aed1044dcc321a9161da"
    );
}

#[test]
fn readback_executor_is_explicitly_wgpu_backend() {
    let executor = rime_native_gpu::WgpuReadbackExecutor::new();

    assert!(
        executor.is_ok()
            || matches!(
                executor,
                Err(rime_native_gpu::WgpuReadbackError::AdapterUnavailable)
            )
    );
}

#[test]
fn readback_rows_use_webgpu_copy_alignment_without_expanding_visible_pixels() {
    assert_eq!(rime_native_gpu::aligned_readback_bytes_per_row(2), 256);
    assert_eq!(
        rime_native_gpu::aligned_readback_bytes_per_row(3744),
        59_904
    );
}
