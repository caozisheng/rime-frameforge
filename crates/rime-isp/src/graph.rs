use crate::{MethodManifest, OperatorDefinition, lcst_producer_by_id, normal_operators};
use rime_core::{
    Extent2d, GraphIqOverride, GraphPresentation, GraphPresentationEdge, GraphTreeKind,
    GraphTreeNode, MethodSpec, NodeExecutionMode, NodeSpec, PipelineManifest, PortRef, PortSpec,
    PreviewPortSpec, PreviewPresentation, ResourceFormat, ScalarType, SignalDomain,
    StatisticsPlaneSpec, StatisticsPortSpec, StatisticsSchema, TemporalEdge,
};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 24;

#[expect(
    clippy::too_many_lines,
    reason = "the fixed manifest is the single explicit topology source"
)]
#[must_use]
///
/// # Panics
///
/// Panics only if the statically registered LCST producer is missing.
pub fn build_normal_manifest() -> PipelineManifest {
    let extent = Extent2d {
        width: WIDTH,
        height: HEIGHT,
    };
    let mut nodes = vec![NodeSpec {
        id: "raw_source".into(),
        display_name: "RAW Source".into(),
        shader_entry: None,
        inputs: Vec::new(),
        outputs: vec![port(
            "out",
            SignalDomain::RawBayerSensor,
            ResourceFormat::R16Uint,
            &extent,
        )],
        statistics_inputs: Vec::new(),
        statistics_outputs: Vec::new(),
        default_method: "fixed_asset".into(),
        methods: Vec::new(),
    }];
    for operator in normal_operators() {
        let definition = operator.definition();
        let method = default_method(definition);
        nodes.push(NodeSpec {
            id: definition.id.into(),
            display_name: definition.label.into(),
            shader_entry: Some(method.shader_entry.into()),
            inputs: vec![port(
                "in",
                method.input.domain,
                method.input.format,
                &extent,
            )],
            outputs: vec![port(
                "out",
                method.output.domain,
                method.output.format,
                &extent,
            )],
            statistics_inputs: if matches!(definition.id, "tintless" | "drc") {
                vec![lcst_statistics_port()]
            } else {
                Vec::new()
            },
            statistics_outputs: Vec::new(),
            default_method: method.method.into(),
            methods: definition
                .methods
                .iter()
                .map(|method| MethodSpec {
                    method: method.method.into(),
                    shader_entry: method.shader_entry.into(),
                    parameters: method
                        .parameters
                        .split_whitespace()
                        .map(Into::into)
                        .collect(),
                })
                .collect(),
        });
    }
    let lcst = lcst_producer_by_id("lcst").expect("registered LCST producer");
    let definition = lcst.definition();
    let method = lcst
        .method(definition.default_method)
        .expect("registered LCST default method");
    nodes.push(NodeSpec {
        id: definition.id.into(),
        display_name: definition.label.into(),
        shader_entry: Some(method.shader.entry_point.into()),
        inputs: vec![port(
            "in",
            SignalDomain::RawBayerRimeQ,
            ResourceFormat::R32Float,
            &extent,
        )],
        outputs: Vec::new(),
        statistics_inputs: Vec::new(),
        statistics_outputs: vec![lcst_statistics_port()],
        default_method: method.method.into(),
        methods: vec![MethodSpec {
            method: method.method.into(),
            shader_entry: method.shader.entry_point.into(),
            parameters: ["width", "height", "cfa_pattern", "d50_gains"]
                .into_iter()
                .map(Into::into)
                .collect(),
        }],
    });
    let preview_outputs = nodes
        .iter()
        .rev()
        .filter_map(|node| {
            node.outputs.first().map(|output| PreviewPortSpec {
                node_id: node.id.clone(),
                port_id: output.id.clone(),
                domain: output.domain,
                format: output.format,
                extent: output.extent.clone(),
                range: if output.domain == SignalDomain::RawBayerSensor {
                    "sensor_code"
                } else {
                    "normalized"
                }
                .into(),
                channel_layout: match output.format {
                    ResourceFormat::R16Uint => "cfa",
                    ResourceFormat::R32Float => "scalar",
                    ResourceFormat::Rgba32Float => "rgba",
                }
                .into(),
                presentation: match output.domain {
                    SignalDomain::RawBayerSensor | SignalDomain::RawBayerRimeQ => {
                        PreviewPresentation::RawGray
                    }
                    SignalDomain::LinearRgb | SignalDomain::EncodedRgb => PreviewPresentation::Rgb,
                    SignalDomain::Yuv => PreviewPresentation::Yuv,
                },
            })
        })
        .collect();

    let chain = [
        "raw_source",
        "blc",
        "sbpc_horizontal",
        "dbpc",
        "sbpc",
        "raw_nr",
        "tintless",
        "lsc",
        "wbc",
        "drc",
        "dem",
        "color_reproduce",
        "rgb2yuv",
    ];
    let mut edges = chain
        .windows(2)
        .enumerate()
        .map(|(index, pair)| TemporalEdge {
            id: format!("normal_edge_{index}"),
            from: PortRef {
                node_id: pair[0].into(),
                port_id: "out".into(),
            },
            to: PortRef {
                node_id: pair[1].into(),
                port_id: "in".into(),
            },
            frame_delay: 0,
        })
        .collect::<Vec<_>>();
    edges.push(TemporalEdge {
        id: "normal_edge_sbpc_lcst".into(),
        from: PortRef {
            node_id: "sbpc".into(),
            port_id: "out".into(),
        },
        to: PortRef {
            node_id: "lcst".into(),
            port_id: "in".into(),
        },
        frame_delay: 0,
    });
    for node_id in ["tintless", "drc"] {
        edges.push(TemporalEdge {
            id: format!("normal_edge_lcst_{node_id}"),
            from: PortRef {
                node_id: "lcst".into(),
                port_id: "lc-stat".into(),
            },
            to: PortRef {
                node_id: node_id.into(),
                port_id: "lc-stat".into(),
            },
            frame_delay: 0,
        });
    }
    let mut manifest = PipelineManifest {
        schema_version: 1,
        graph_id: "normal".into(),
        graph_kind: "video-isp/normal".into(),
        manifest_hash: String::new(),
        nodes,
        edges,
        preview_outputs,
    };
    manifest.refresh_hash();
    manifest
}

#[must_use]
pub fn build_normal_graph_presentation() -> GraphPresentation {
    let manifest = build_normal_manifest();
    let mut nodes = vec![group(
        "normal",
        "normal",
        None,
        NodeExecutionMode::Enabled,
        true,
    )];
    nodes.push(endpoint(
        "raw_source",
        "RAW Source",
        NodeExecutionMode::Enabled,
        Some("raw_source"),
        Vec::new(),
        vec!["out"],
    ));
    nodes.extend(vfe_nodes());
    nodes.extend(vbe_nodes());
    nodes.extend(vpe_nodes());
    nodes.push(endpoint(
        "encoder",
        "FFmpeg Encoder",
        NodeExecutionMode::Disabled,
        None,
        vec!["in"],
        Vec::new(),
    ));
    hydrate_executable_ports(&mut nodes, &manifest);
    GraphPresentation {
        graph_id: "normal".into(),
        root_id: "normal".into(),
        nodes,
        iq_overrides: vec![
            GraphIqOverride {
                id: "mctf_1".into(),
                module_id: "mctf".into(),
            },
            GraphIqOverride {
                id: "mctf_2".into(),
                module_id: "mctf".into(),
            },
        ],
        edges: presentation_edges(),
    }
}

fn hydrate_executable_ports(nodes: &mut [GraphTreeNode], manifest: &PipelineManifest) {
    for node in nodes {
        let Some(execution_node_id) = node.execution_node_id.as_deref() else {
            continue;
        };
        let Some(manifest_node) = manifest.node(execution_node_id) else {
            continue;
        };
        node.inputs = manifest_node
            .inputs
            .iter()
            .map(|port| port.id.clone())
            .chain(
                manifest_node
                    .statistics_inputs
                    .iter()
                    .map(|port| port.id.clone()),
            )
            .collect();
        node.outputs = manifest_node
            .outputs
            .iter()
            .map(|port| port.id.clone())
            .chain(
                manifest_node
                    .statistics_outputs
                    .iter()
                    .map(|port| port.id.clone()),
            )
            .collect();
    }
}

fn default_method(operator: &OperatorDefinition) -> &MethodManifest {
    operator
        .methods
        .iter()
        .find(|method| method.method == operator.default_method)
        .expect("operator default method must be registered")
}

fn port(id: &str, domain: SignalDomain, format: ResourceFormat, extent: &Extent2d) -> PortSpec {
    PortSpec {
        id: id.into(),
        domain,
        format,
        extent: extent.clone(),
    }
}
fn lcst_statistics_port() -> StatisticsPortSpec {
    StatisticsPortSpec {
        id: "lc-stat".into(),
        schema: StatisticsSchema::Lcst {
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
        },
    }
}

fn vfe_nodes() -> Vec<GraphTreeNode> {
    vec![
        group(
            "vfe",
            "VFE",
            Some("normal"),
            NodeExecutionMode::Enabled,
            true,
        ),
        operator(
            "blc",
            "BLC",
            "vfe",
            NodeExecutionMode::Enabled,
            Some("blc"),
            None,
        ),
        operator(
            "sbpc_horizontal",
            "SBPC-H",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("sbpc_horizontal"),
            Some("method 00: identity bypass"),
        ),
        operator(
            "dbpc",
            "DBPC",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("dbpc"),
            Some("method 00: identity bypass"),
        ),
        operator(
            "sbpc",
            "SBPC",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("sbpc"),
            Some("Static Bad Pixel Correction; method 00: identity bypass"),
        ),
        statistics_operator(
            "pdafst",
            "PDAFST",
            "vfe",
            "pdaf-stat",
            "phase-difference AF statistics placeholder; output pdaf-stat",
        ),
        enabled_statistics_operator("lcst", "LCST", "vfe", "lcst", "lc-stat"),
        statistics_operator(
            "cdafst",
            "CDAFST",
            "vfe",
            "cdaf-stat",
            "contrast-difference AF statistics placeholder; output cdaf-stat",
        ),
        operator(
            "raw_nr",
            "RAW-NR",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("raw_nr"),
            Some("RAW-domain denoise; method 00: identity bypass"),
        ),
    ]
}

fn statistics_operator(
    id: &str,
    label: &str,
    parent: &str,
    output: &str,
    reason: &str,
) -> GraphTreeNode {
    let mut node = operator(
        id,
        label,
        parent,
        NodeExecutionMode::Disabled,
        None,
        Some(reason),
    );
    node.outputs = vec![output.into()];
    node
}
fn enabled_statistics_operator(
    id: &str,
    label: &str,
    parent: &str,
    execution_node_id: &str,
    output: &str,
) -> GraphTreeNode {
    let mut node = operator(
        id,
        label,
        parent,
        NodeExecutionMode::Enabled,
        Some(execution_node_id),
        None,
    );
    node.outputs = vec![output.into()];
    node
}
fn vbe_nodes() -> Vec<GraphTreeNode> {
    vec![
        group(
            "vbe",
            "VBE",
            Some("normal"),
            NodeExecutionMode::Enabled,
            true,
        ),
        operator(
            "tintless",
            "TINTLESS",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("tintless"),
            None,
        ),
        operator(
            "lsc",
            "LSC",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("lsc"),
            None,
        ),
        operator(
            "wbc",
            "WBC",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("wbc"),
            None,
        ),
        operator(
            "drc",
            "DRC",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("drc"),
            None,
        ),
        operator(
            "dem",
            "DEM",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("dem"),
            None,
        ),
        operator(
            "color_reproduce",
            "Color Reproduce",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("color_reproduce"),
            None,
        ),
        operator(
            "rgb2yuv",
            "RGB2YUV",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("rgb2yuv"),
            None,
        ),
        operator(
            "pyrd",
            "PYRD",
            "vbe",
            NodeExecutionMode::Disabled,
            None,
            Some("Gaussian Pyramid Decomposition; one input, three scale outputs"),
        ),
    ]
}

fn vpe_nodes() -> Vec<GraphTreeNode> {
    let mut nodes = vec![group(
        "vpe",
        "VPE",
        Some("normal"),
        NodeExecutionMode::Disabled,
        false,
    )];
    for (prefix, label) in [
        ("vpe_16", "pass-1"),
        ("vpe_4", "pass-2"),
        ("vpe_full", "pass-3"),
    ] {
        let group_id = format!("{prefix}_pass");
        nodes.push(group(
            &group_id,
            label,
            Some("vpe"),
            NodeExecutionMode::Disabled,
            true,
        ));
        for (suffix, name) in [
            ("pyrc", "PYRC"),
            ("mctf_1", "MCTF"),
            ("lce", "LCE"),
            ("ce", "CE"),
            ("mctf_2", "MCTF"),
            ("sharpen", "Sharpen"),
            ("sharpen", "Sharpen"),
        ] {
            let mut node = operator(
                &format!("{prefix}_{suffix}"),
                name,
                &group_id,
                NodeExecutionMode::Disabled,
                None,
                Some("pyramid input unavailable"),
            );
            if suffix == "mctf_1" || suffix == "mctf_2" {
                node.module_id = Some("mctf".into());
                node.iq_override_id = Some(suffix.into());
            }
            nodes.push(node);
        }
    }
    nodes
}

fn presentation_edges() -> Vec<GraphPresentationEdge> {
    let pairs = [
        ("raw_source", "out", "blc", "in", None),
        ("blc", "out", "sbpc_horizontal", "in", None),
        ("sbpc_horizontal", "out", "dbpc", "in", None),
        ("sbpc_horizontal", "out", "pdafst", "in", None),
        ("dbpc", "out", "sbpc", "in", None),
        ("sbpc", "out", "raw_nr", "in", None),
        ("sbpc", "out", "lcst", "in", None),
        (
            "lcst",
            "lc-stat",
            "tintless",
            "lc-stat",
            Some("LCST statistics"),
        ),
        ("lcst", "lc-stat", "drc", "lc-stat", Some("LCST statistics")),
        ("sbpc", "out", "cdafst", "in", None),
        ("raw_nr", "out", "tintless", "in", None),
        ("tintless", "out", "lsc", "in", None),
        ("lsc", "out", "wbc", "in", None),
        ("wbc", "out", "drc", "in", None),
        ("drc", "out", "dem", "in", None),
        ("dem", "out", "color_reproduce", "in", None),
        ("color_reproduce", "out", "rgb2yuv", "in", None),
        ("rgb2yuv", "out", "pyrd", "in", None),
        ("vpe_16_sharpen", "out", "vpe_4_pyrc", "feedback", None),
        ("vpe_4_sharpen", "out", "vpe_full_pyrc", "feedback", None),
        ("vpe_full_sharpen", "out", "encoder", "in", None),
        ("pyrd", "full", "vpe_full_pyrc", "in", Some("Full YUV")),
        ("pyrd", "quarter", "vpe_4_pyrc", "in", Some("1/4 YUV")),
        ("pyrd", "sixteenth", "vpe_16_pyrc", "in", Some("1/16 YUV")),
    ];
    let mut edges: Vec<GraphPresentationEdge> = pairs
        .into_iter()
        .enumerate()
        .map(
            |(index, (from, from_port, to, to_port, label))| GraphPresentationEdge {
                id: format!("normal_edge_{index}"),
                from: from.into(),
                to: to.into(),
                from_port: from_port.into(),
                to_port: to_port.into(),
                label: label.map(str::to_owned),
            },
        )
        .collect();
    for prefix in ["vpe_16", "vpe_4", "vpe_full"] {
        for (index, (from, to)) in [
            ("pyrc", "mctf_1"),
            ("mctf_1", "lce"),
            ("lce", "ce"),
            ("ce", "mctf_2"),
            ("mctf_2", "sharpen"),
        ]
        .into_iter()
        .enumerate()
        {
            edges.push(GraphPresentationEdge {
                id: format!("normal_edge_vpe_{prefix}_{index}"),
                from: format!("{prefix}_{from}"),
                to: format!("{prefix}_{to}"),
                from_port: "out".into(),
                to_port: "in".into(),
                label: None,
            });
        }
    }
    edges
}

fn group(
    id: &str,
    label: &str,
    parent: Option<&str>,
    mode: NodeExecutionMode,
    expanded: bool,
) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: parent.map(str::to_owned),
        kind: GraphTreeKind::Group,
        mode,
        execution_node_id: None,
        module_id: None,
        iq_override_id: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        reason: None,
        default_expanded: expanded,
    }
}

fn endpoint(
    id: &str,
    label: &str,
    mode: NodeExecutionMode,
    execution_node_id: Option<&str>,
    inputs: Vec<&str>,
    outputs: Vec<&str>,
) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: Some("normal".into()),
        kind: GraphTreeKind::Endpoint,
        mode,
        execution_node_id: execution_node_id.map(str::to_owned),
        module_id: None,
        iq_override_id: None,
        inputs: inputs.into_iter().map(str::to_owned).collect(),
        outputs: outputs.into_iter().map(str::to_owned).collect(),
        reason: None,
        default_expanded: false,
    }
}

fn operator(
    id: &str,
    label: &str,
    parent: &str,
    mode: NodeExecutionMode,
    execution_node_id: Option<&str>,
    reason: Option<&str>,
) -> GraphTreeNode {
    GraphTreeNode {
        id: id.into(),
        label: label.into(),
        parent_id: Some(parent.into()),
        kind: GraphTreeKind::Operator,
        mode,
        execution_node_id: execution_node_id.map(str::to_owned),
        module_id: None,
        iq_override_id: None,
        inputs: if id.ends_with("_pyrc") {
            vec!["in".into(), "feedback".into()]
        } else {
            vec!["in".into()]
        },
        outputs: if id == "pyrd" {
            vec!["full".into(), "quarter".into(), "sixteenth".into()]
        } else {
            vec!["out".into()]
        },
        reason: reason.map(str::to_owned),
        default_expanded: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_presentation_ports_follow_the_manifest() {
        let mut manifest = build_normal_manifest();
        manifest
            .nodes
            .iter_mut()
            .find(|node| node.id == "blc")
            .expect("BLC node")
            .inputs[0]
            .id = "manifest_input".into();
        let mut nodes = vec![operator(
            "blc",
            "BLC",
            "vfe",
            NodeExecutionMode::Enabled,
            Some("blc"),
            None,
        )];

        hydrate_executable_ports(&mut nodes, &manifest);

        assert_eq!(nodes[0].inputs, ["manifest_input"]);
    }
}
