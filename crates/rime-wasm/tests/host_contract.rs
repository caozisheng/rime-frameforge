use rime_wasm::NormalRuntime;

#[test]
fn wasm_runtime_exposes_the_normal_manifest() {
    let runtime = NormalRuntime::new();

    assert!(runtime.manifest_json().contains("\"graph_id\":\"normal\""));
}

#[test]
fn wasm_runtime_step_exposes_warmup_snapshot() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");

    let snapshot = runtime.step().expect("step may start");

    assert!(snapshot.contains("\"frame_phase\":\"warmup\""));
}

#[test]
fn wasm_runtime_failure_enters_error() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");

    let snapshot = runtime.fail();

    assert!(snapshot.contains("\"lifecycle_state\":\"error\""));
}

#[test]
fn wasm_runtime_accepts_quantization_config_and_increments_revision() {
    let mut runtime = NormalRuntime::new();
    runtime.load().expect("built-in manifest must load");
    let graph = rime_isp::build_normal_graph_presentation();
    let mut config = rime_core::GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.enabled = false;
    let snapshot = runtime
        .set_quantization_config(&serde_json::to_string(&config).expect("config JSON"))
        .expect("valid config");
    assert!(snapshot.contains("\"config_revision\":1"));
}
