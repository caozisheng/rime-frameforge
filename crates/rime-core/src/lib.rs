#![forbid(unsafe_code)]

mod compiled_graph;
mod diagnostic;
mod generated_assets;
mod graph_presentation;
mod manifest;
mod runtime_state;

pub use compiled_graph::CompiledGraph;
pub use diagnostic::{Diagnostic, DiagnosticCode};
pub use generated_assets::render_top_graph_presentation_typescript;
pub use graph_presentation::{
    GraphIqOverride, GraphPresentation, GraphPresentationEdge, GraphTreeKind, GraphTreeNode,
    IqParameterSource, IqResolutionError, NodeExecutionMode, build_top_graph_presentation,
};
pub use manifest::{
    Extent2d, MethodSpec, NodeSpec, PipelineManifest, PortRef, PortSpec, PreviewPortSpec,
    ResourceFormat, SignalDomain, TemporalEdge,
};
pub use runtime_state::{FramePhase, GraphRuntime, LifecycleState, RuntimeSnapshot};
