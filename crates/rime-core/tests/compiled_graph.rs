use rime_core::{
    CompiledGraph, DiagnosticCode, Extent2d, NodeSpec, PipelineManifest, PortRef, PortSpec,
    ResourceFormat, SignalDomain, TemporalEdge,
};

#[test]
fn reverse_reachability_collects_the_complete_path() {
    let manifest = graph_fixture();
    let graph = CompiledGraph::new(&manifest).expect("fixture graph must compile");

    let order = graph
        .execution_order_for_outputs(&["output"])
        .expect("final output must resolve");

    assert_eq!(order, ["source", "output"]);
}

#[test]
fn unknown_output_root_returns_a_stable_manifest_error() {
    let manifest = graph_fixture();
    let graph = CompiledGraph::new(&manifest).expect("fixture graph must compile");

    let error = graph
        .execution_order_for_outputs(&["missing"])
        .expect_err("unknown output root must fail");

    assert_eq!(error.code, DiagnosticCode::ManifestInvalid);
}

fn graph_fixture() -> PipelineManifest {
    let extent = Extent2d {
        width: 1,
        height: 1,
    };
    let source_output = PortSpec {
        id: "out".into(),
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::R32Float,
        extent: extent.clone(),
    };
    let output_input = PortSpec {
        id: "in".into(),
        ..source_output.clone()
    };
    PipelineManifest {
        schema_version: 1,
        graph_id: "fixture".into(),
        graph_kind: "test/fixture".into(),
        manifest_hash: String::new(),
        nodes: vec![
            NodeSpec {
                id: "source".into(),
                display_name: "Source".into(),
                shader_entry: None,
                inputs: Vec::new(),
                outputs: vec![source_output],
                default_method: "source".into(),
                methods: Vec::new(),
            },
            NodeSpec {
                id: "output".into(),
                display_name: "Output".into(),
                shader_entry: Some("output_main".into()),
                inputs: vec![output_input],
                outputs: vec![PortSpec {
                    id: "out".into(),
                    domain: SignalDomain::RawBayerRimeQ,
                    format: ResourceFormat::R32Float,
                    extent,
                }],
                default_method: "00".into(),
                methods: Vec::new(),
            },
        ],
        edges: vec![TemporalEdge {
            id: "edge".into(),
            from: PortRef {
                node_id: "source".into(),
                port_id: "out".into(),
            },
            to: PortRef {
                node_id: "output".into(),
                port_id: "in".into(),
            },
            frame_delay: 0,
        }],
        preview_outputs: Vec::new(),
    }
}
