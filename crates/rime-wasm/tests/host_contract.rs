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
