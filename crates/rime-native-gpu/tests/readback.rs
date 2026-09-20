use std::path::Path;

use rime_core::FramePhase;
use rime_dng::DngReader;
use rime_isp::vbe::drc::DrcExposurePolicy;
use rime_native_gpu::{
    NativeFrameIdentity, RenderFeatureFlags, WgpuReadbackError, WgpuReadbackExecutor,
};

const GH5S_SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../pipeline/normal/P1020601.dng"
);

#[test]
fn gh5s_frame_runs_through_native_operator_graph() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 7)
        .expect("GH5S DNG must decode");
    let executor = match WgpuReadbackExecutor::new() {
        Ok(executor) => executor,
        Err(WgpuReadbackError::AdapterUnavailable) => return,
        Err(error) => panic!("native GPU must initialize when an adapter exists: {error}"),
    };

    let surface = executor
        .render(&frame)
        .expect("native operator graph must read back");

    assert_eq!(surface.width(), frame.layout.width);
    assert_eq!(surface.height(), frame.layout.height);
    assert_eq!(surface.identity().frame_index, 7);
    assert_eq!(
        surface.pixels().len(),
        (frame.layout.width * frame.layout.height * 4) as usize
    );
    assert!(surface.pixels().iter().all(|value| value.is_finite()));
}

#[test]
fn gh5s_frame_can_select_local_tone_drc01() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 8)
        .expect("GH5S DNG must decode");
    let executor = match WgpuReadbackExecutor::new() {
        Ok(executor) => executor,
        Err(WgpuReadbackError::AdapterUnavailable) => return,
        Err(error) => panic!("native GPU must initialize when an adapter exists: {error}"),
    };

    let global = executor
        .render_with_drc_method(&frame, "00")
        .expect("DRC00 render");
    let local = executor
        .render_with_drc_method(&frame, "01")
        .expect("DRC01 render");

    assert_eq!(local.width(), global.width());
    assert_eq!(local.height(), global.height());
    assert!(local.pixels().iter().all(|value| value.is_finite()));
    assert!(
        local
            .pixels()
            .iter()
            .zip(global.pixels())
            .any(|(left, right)| (left - right).abs() > 1e-6),
        "local tone must differ from global tone on the GH5S frame"
    );
}

#[test]
fn gh5s_sequence_frame_zero_cold_starts_drc01() {
    let frame = DngReader::new()
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S frame zero must decode");
    let executor = match WgpuReadbackExecutor::new() {
        Ok(executor) => executor,
        Err(WgpuReadbackError::AdapterUnavailable) => return,
        Err(error) => panic!("native GPU must initialize when an adapter exists: {error}"),
    };
    let identity = NativeFrameIdentity {
        frame_index: 0,
        run_revision: 4,
        method_revision: 2,
        gpu_generation: 1,
        phase: FramePhase::Output,
    };

    let output = executor
        .render_sequence_frame_with_options(
            &frame,
            identity,
            None,
            "01",
            DrcExposurePolicy::Baseline,
            None,
            0.0,
            RenderFeatureFlags::default(),
        )
        .expect("sequence frame zero must cold-start DRC01 without LCST history");

    assert_eq!(output.surface().identity(), identity);
    assert!(
        output
            .surface()
            .pixels()
            .iter()
            .all(|value| value.is_finite())
    );
}

#[test]
fn gh5s_sequence_uses_previous_frame_lcst_statistics() {
    let reader = DngReader::new();
    let frame_zero = reader
        .decode_file(Path::new(GH5S_SAMPLE), 0)
        .expect("GH5S frame zero must decode");
    let frame_one = reader
        .decode_file(Path::new(GH5S_SAMPLE), 1)
        .expect("GH5S frame one must decode");
    let executor = match WgpuReadbackExecutor::new() {
        Ok(executor) => executor,
        Err(WgpuReadbackError::AdapterUnavailable) => return,
        Err(error) => panic!("native GPU must initialize when an adapter exists: {error}"),
    };
    let identity = |frame_index| NativeFrameIdentity {
        frame_index,
        run_revision: 4,
        method_revision: 2,
        gpu_generation: 1,
        phase: FramePhase::Output,
    };

    let first = executor
        .render_sequence_frame(&frame_zero, identity(0), None)
        .expect("sequence frame zero cold start");
    let second = executor
        .render_sequence_frame(&frame_one, identity(1), Some(first.statistics()))
        .expect("sequence frame one consumes frame zero statistics");

    assert_eq!(first.statistics().identity().frame_index, 0);
    assert_eq!(second.statistics().identity().frame_index, 1);
    assert_eq!(second.surface().identity().frame_index, 1);
    assert!(
        second
            .surface()
            .pixels()
            .iter()
            .all(|value| value.is_finite())
    );
}
