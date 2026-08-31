use serde::{Deserialize, Serialize};

use crate::{Diagnostic, DiagnosticCode};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    #[default]
    Unloaded,
    Loading,
    Stop,
    Running,
    Stepping,
    Paused,
    Completed,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FramePhase {
    Warmup,
    Output,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSnapshot {
    pub lifecycle_state: LifecycleState,
    pub run_revision: u64,
    pub method_revision: u64,
    pub config_revision: u64,
    pub gpu_generation: u64,
    pub frame_index: Option<u64>,
    pub frame_phase: Option<FramePhase>,
    pub visible_frame: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct GraphRuntime {
    run_revision: u64,
    method_revision: u64,
    config_revision: u64,
    gpu_generation: u64,
    frame_index: Option<u64>,
    frame_phase: Option<FramePhase>,
    visible_frame: Option<u64>,
    lifecycle_state: LifecycleState,
}

impl GraphRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            lifecycle_state: self.lifecycle_state,
            run_revision: self.run_revision,
            method_revision: self.method_revision,
            config_revision: self.config_revision,
            gpu_generation: self.gpu_generation,
            frame_index: self.frame_index,
            frame_phase: self.frame_phase,
            visible_frame: self.visible_frame,
        }
    }

    /// Begins loading from an unloaded or failed graph.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` outside `Unloaded` or `Error`.
    pub fn begin_load(&mut self) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Unloaded, LifecycleState::Error])?;
        self.lifecycle_state = LifecycleState::Loading;
        Ok(())
    }

    /// Commits a successful graph load and enters `Stop`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless loading is active.
    pub fn load_succeeded(&mut self) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Loading])?;
        self.lifecycle_state = LifecycleState::Stop;
        self.method_revision = self.method_revision.saturating_add(1);
        Ok(())
    }

    /// Starts or resumes execution.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless execution can start or resume.
    pub fn run(&mut self) -> Result<(), Diagnostic> {
        self.run_frame(0)
    }

    /// Starts or resumes execution for a logical source frame.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless execution can start or resume.
    pub fn run_frame(&mut self, frame_index: u64) -> Result<(), Diagnostic> {
        self.require_state(&[
            LifecycleState::Stop,
            LifecycleState::Paused,
            LifecycleState::Completed,
        ])?;
        if matches!(
            self.lifecycle_state,
            LifecycleState::Stop | LifecycleState::Completed
        ) {
            self.start_new_run();
        }
        self.lifecycle_state = LifecycleState::Running;
        self.frame_index = Some(frame_index);
        self.frame_phase = Some(FramePhase::Warmup);
        Ok(())
    }

    /// Starts one visible-frame step.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` outside `Stop` or `Paused`.
    pub fn step(&mut self) -> Result<(), Diagnostic> {
        self.step_frame(0)
    }

    /// Starts one visible-frame step for a logical source frame.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` outside `Stop` or `Paused`.
    pub fn step_frame(&mut self, frame_index: u64) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Stop, LifecycleState::Paused])?;
        if self.lifecycle_state == LifecycleState::Stop {
            self.start_new_run();
        }
        self.lifecycle_state = LifecycleState::Stepping;
        self.frame_index = Some(frame_index);
        self.frame_phase = Some(FramePhase::Warmup);
        Ok(())
    }

    /// Advances a real warmup pass to the output phase.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless warmup is executing.
    pub fn complete_warmup(&mut self) -> Result<(), Diagnostic> {
        self.require_executing(FramePhase::Warmup)?;
        self.frame_phase = Some(FramePhase::Output);
        Ok(())
    }

    /// Commits the active logical frame after the output pass.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless output is executing.
    pub fn complete_output(&mut self) -> Result<(), Diagnostic> {
        self.require_executing(FramePhase::Output)?;
        self.visible_frame = self.frame_index;
        self.frame_phase = None;
        self.lifecycle_state = LifecycleState::Completed;
        Ok(())
    }

    /// Invalidates runtime resources and returns the loaded graph to `Stop`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` before a graph has been loaded.
    pub fn reset(&mut self) -> Result<(), Diagnostic> {
        self.require_state(&[
            LifecycleState::Stop,
            LifecycleState::Running,
            LifecycleState::Stepping,
            LifecycleState::Paused,
            LifecycleState::Completed,
            LifecycleState::Error,
        ])?;
        self.gpu_generation = self.gpu_generation.saturating_add(1);
        self.frame_index = None;
        self.frame_phase = None;
        self.visible_frame = None;
        self.lifecycle_state = LifecycleState::Stop;
        Ok(())
    }

    /// Invalidates the visible frame after a method or parameter change.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless the graph is stopped or completed.
    pub fn change_method(&mut self) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Stop, LifecycleState::Completed])?;
        self.method_revision = self.method_revision.saturating_add(1);
        self.frame_index = None;
        self.frame_phase = None;
        self.visible_frame = None;
        self.lifecycle_state = LifecycleState::Stop;
        Ok(())
    }

    /// Invalidates output after a graph configuration change.
    ///
    /// # Errors
    ///
    /// Returns `InvalidStateTransition` unless the graph is stopped or completed.
    pub fn change_config(&mut self) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Stop, LifecycleState::Completed])?;
        self.config_revision = self.config_revision.saturating_add(1);
        self.frame_index = None;
        self.frame_phase = None;
        self.visible_frame = None;
        self.lifecycle_state = LifecycleState::Stop;
        Ok(())
    }

    pub fn fail(&mut self) {
        self.frame_phase = None;
        self.lifecycle_state = LifecycleState::Error;
    }

    pub fn device_lost(&mut self) {
        self.gpu_generation = self.gpu_generation.saturating_add(1);
        self.visible_frame = None;
        self.fail();
    }

    fn start_new_run(&mut self) {
        self.run_revision = self.run_revision.saturating_add(1);
        self.visible_frame = None;
    }

    fn require_executing(&self, phase: FramePhase) -> Result<(), Diagnostic> {
        self.require_state(&[LifecycleState::Running, LifecycleState::Stepping])?;
        if self.frame_phase != Some(phase) {
            return Err(invalid_transition(self.lifecycle_state, "complete phase"));
        }
        Ok(())
    }

    fn require_state(&self, allowed: &[LifecycleState]) -> Result<(), Diagnostic> {
        if allowed.contains(&self.lifecycle_state) {
            Ok(())
        } else {
            Err(invalid_transition(self.lifecycle_state, "apply command"))
        }
    }
}

fn invalid_transition(state: LifecycleState, command: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::InvalidStateTransition,
        format!("cannot {command} while graph is {state:?}"),
    )
}
