#![expect(
    dead_code,
    reason = "the command module is included to exercise native descriptor serialization"
)]

#[path = "../src/dng_command.rs"]
mod dng_command;
#[path = "../src/native_pipeline.rs"]
mod native_pipeline;

#[test]
fn native_render_descriptor_preserves_preview_and_fallback_contract() {
    let descriptor = native_pipeline::NativeRenderDescriptor {
        frame_index: 7,
        width: 3744,
        height: 2776,
        node_id: "rgb2yuv",
        port_id: "out",
        encoder_backend: "cpu_readback",
        preview_data_url: "data:image/png;base64,preview".to_owned(),
    };

    let json = serde_json::to_value(descriptor).expect("descriptor serializes");
    assert_eq!(json["frameIndex"], 7);
    assert_eq!(json["nodeId"], "rgb2yuv");
    assert_eq!(json["portId"], "out");
    assert_eq!(json["encoderBackend"], "cpu_readback");
}
