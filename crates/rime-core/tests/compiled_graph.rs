use rime_core::{
    CompiledGraph, DiagnosticCode, Extent2d, NodeSpec, PipelineManifest, PortRef, PortSpec,
    ResourceFormat, ScalarType, SignalDomain, StatisticsPlaneSpec, StatisticsPortSpec,
    StatisticsSchema, TemporalEdge,
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

#[test]
fn typed_statistics_edge_accepts_matching_lcst_schema_and_frame_delay() {
    let mut manifest = statistics_graph_fixture(lcst_schema(), lcst_schema(), 1);
    manifest.refresh_hash();

    assert_eq!(manifest.validate(), Ok(()));
    assert_eq!(manifest.edges[0].frame_delay, 1);
}

#[test]
fn typed_statistics_edge_rejects_schema_mismatch() {
    let mut consumer_schema = lcst_schema();
    let StatisticsSchema::Lcst { average_rggb, .. } = &mut consumer_schema;
    average_rggb.width = 32;
    let mut manifest = statistics_graph_fixture(lcst_schema(), consumer_schema, 0);
    manifest.refresh_hash();

    let error = manifest
        .validate()
        .expect_err("mismatched statistics schema must fail");
    assert_eq!(error.code, DiagnosticCode::PortContractMismatch);
}

#[test]
fn image_port_cannot_connect_to_statistics_port() {
    let mut manifest = statistics_graph_fixture(lcst_schema(), lcst_schema(), 0);
    manifest.nodes[0].statistics_outputs.clear();
    manifest.nodes[0].outputs.push(PortSpec {
        id: "lc-stat".into(),
        domain: SignalDomain::RawBayerRimeQ,
        format: ResourceFormat::Rgba32Float,
        extent: Extent2d {
            width: 64,
            height: 48,
        },
    });
    manifest.refresh_hash();

    let error = manifest
        .validate()
        .expect_err("image/statistics edge must fail");
    assert_eq!(error.code, DiagnosticCode::PortContractMismatch);
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
                statistics_inputs: Vec::new(),
                statistics_outputs: Vec::new(),
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
                statistics_inputs: Vec::new(),
                statistics_outputs: Vec::new(),
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

fn lcst_schema() -> StatisticsSchema {
    StatisticsSchema::Lcst {
        average_rggb: StatisticsPlaneSpec {
            width: 64,
            height: 48,
            channels: 4,
            scalar: ScalarType::F32,
        },
        luma_histogram: StatisticsPlaneSpec {
            width: 16,
            height: 16,
            channels: 16,
            scalar: ScalarType::U32,
        },
    }
}

fn statistics_graph_fixture(
    producer_schema: StatisticsSchema,
    consumer_schema: StatisticsSchema,
    frame_delay: u32,
) -> PipelineManifest {
    PipelineManifest {
        schema_version: 1,
        graph_id: "statistics-fixture".into(),
        graph_kind: "test/statistics".into(),
        manifest_hash: String::new(),
        nodes: vec![
            NodeSpec {
                id: "producer".into(),
                display_name: "Producer".into(),
                shader_entry: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
                statistics_inputs: Vec::new(),
                statistics_outputs: vec![StatisticsPortSpec {
                    id: "lc-stat".into(),
                    schema: producer_schema,
                }],
                default_method: "producer".into(),
                methods: Vec::new(),
            },
            NodeSpec {
                id: "consumer".into(),
                display_name: "Consumer".into(),
                shader_entry: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
                statistics_inputs: vec![StatisticsPortSpec {
                    id: "lc-stat".into(),
                    schema: consumer_schema,
                }],
                statistics_outputs: Vec::new(),
                default_method: "consumer".into(),
                methods: Vec::new(),
            },
        ],
        edges: vec![TemporalEdge {
            id: "statistics-edge".into(),
            from: PortRef {
                node_id: "producer".into(),
                port_id: "lc-stat".into(),
            },
            to: PortRef {
                node_id: "consumer".into(),
                port_id: "lc-stat".into(),
            },
            frame_delay,
        }],
        preview_outputs: Vec::new(),
    }
}
