use rime_core::{FramePhase, GraphRuntime, LifecycleState};

#[test]
fn successful_load_enters_stop() {
    let mut runtime = GraphRuntime::new();

    runtime.begin_load().expect("load may begin");
    runtime.load_succeeded().expect("valid graph may load");

    assert_eq!(runtime.snapshot().lifecycle_state, LifecycleState::Stop);
}

#[test]
fn step_starts_frame_zero_warmup() {
    let mut runtime = loaded_runtime();

    runtime.step().expect("step may start from stop");

    assert_eq!(runtime.snapshot().frame_phase, Some(FramePhase::Warmup));
}

#[test]
fn warmup_completion_does_not_commit_a_visible_frame() {
    let mut runtime = loaded_runtime();
    runtime.step().expect("step may start");

    runtime.complete_warmup().expect("warmup may complete");

    assert_eq!(runtime.snapshot().visible_frame, None);
}

#[test]
fn output_completion_commits_frame_zero_and_completes() {
    let mut runtime = loaded_runtime();
    runtime.step().expect("step may start");
    runtime.complete_warmup().expect("warmup may complete");

    runtime.complete_output().expect("output may complete");

    assert_eq!(
        (
            runtime.snapshot().lifecycle_state,
            runtime.snapshot().visible_frame
        ),
        (LifecycleState::Completed, Some(0))
    );
}

#[test]
fn replay_increments_run_revision() {
    let mut runtime = completed_runtime();
    let completed_revision = runtime.snapshot().run_revision;

    runtime.run().expect("completed graph may replay");

    assert_eq!(runtime.snapshot().run_revision, completed_revision + 1);
}

#[test]
fn reset_invalidates_old_gpu_generation() {
    let mut runtime = completed_runtime();
    let completed_generation = runtime.snapshot().gpu_generation;

    runtime.reset().expect("completed graph may reset");

    assert_eq!(runtime.snapshot().gpu_generation, completed_generation + 1);
}
#[test]
fn method_change_invalidates_visible_frame_and_increments_revision() {
    let mut runtime = completed_runtime();
    let revision = runtime.snapshot().method_revision;

    runtime.change_method().expect("completed graph method may change");

    assert_eq!(runtime.snapshot().method_revision, revision + 1);
    assert_eq!(runtime.snapshot().visible_frame, None);
    assert_eq!(runtime.snapshot().lifecycle_state, LifecycleState::Stop);
}
#[test]
fn quantization_change_in_stop_increments_config_revision() {
    let mut runtime = loaded_runtime();
    let revision = runtime.snapshot().config_revision;

    runtime.change_config().expect("config may change while stopped");

    assert_eq!(runtime.snapshot().config_revision, revision + 1);
}

#[test]
fn quantization_change_is_rejected_while_running() {
    let mut runtime = loaded_runtime();
    runtime.step().expect("step may start");

    assert!(runtime.change_config().is_err());
}

fn loaded_runtime() -> GraphRuntime {
    let mut runtime = GraphRuntime::new();
    runtime.begin_load().expect("load may begin");
    runtime.load_succeeded().expect("load may finish");
    runtime
}

#[test]
fn device_loss_enters_error_and_invalidates_gpu_generation() {
    let mut runtime = loaded_runtime();
    let generation = runtime.snapshot().gpu_generation;

    runtime.device_lost();

    assert_eq!(
        (
            runtime.snapshot().lifecycle_state,
            runtime.snapshot().gpu_generation
        ),
        (LifecycleState::Error, generation + 1)
    );
}
fn completed_runtime() -> GraphRuntime {
    let mut runtime = loaded_runtime();
    runtime.step().expect("step may start");
    runtime.complete_warmup().expect("warmup may complete");
    runtime.complete_output().expect("output may complete");
    runtime
}
