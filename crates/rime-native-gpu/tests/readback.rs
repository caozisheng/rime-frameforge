use std::path::Path;

use rime_dng::DngReader;
use rime_native_gpu::{WgpuReadbackError, WgpuReadbackExecutor};

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
