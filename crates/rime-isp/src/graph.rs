use crate::{OperatorDefinition, OperatorMethod, normal_operators};
use rime_core::{
    Extent2d, GraphIqOverride, GraphPresentation, GraphPresentationEdge, GraphTreeKind,
    GraphTreeNode, MethodSpec, NodeExecutionMode, NodeSpec, PipelineManifest, PortRef, PortSpec,
    PreviewPortSpec, PreviewPresentation, ResourceFormat, SignalDomain, TemporalEdge,
};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 24;

#[expect(
    clippy::too_many_lines,
    reason = "the fixed manifest is the single explicit topology source"
)]
#[must_use]
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
                definition.input.domain,
                definition.input.format,
                &extent,
            )],
            outputs: vec![port(
                "out",
                definition.output.domain,
                definition.output.format,
                &extent,
            )],
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
        "tintless",
        "lsc",
        "hr",
        "drc",
        "cac",
        "raw_nr",
        "wbc",
        "dem",
        "pfr",
        "color_correction",
        "gamma",
        "three_d_lut",
        "rgb2yuv",
    ];
    let edges = chain
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
        .collect();
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
            .collect();
        node.outputs = manifest_node
            .outputs
            .iter()
            .map(|port| port.id.clone())
            .collect();
    }
}

fn default_method(operator: &OperatorDefinition) -> &OperatorMethod {
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
        operator(
            "tintless",
            "TINTLESS",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("tintless"),
            Some("color shading correction; method 00: identity bypass"),
        ),
        operator(
            "lsc",
            "LSC",
            "vfe",
            NodeExecutionMode::Bypass,
            Some("lsc"),
            Some("luma shading correction; method 00: identity bypass"),
        ),
    ]
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixed presentation keeps the architecture order explicit"
)]
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
            "hr",
            "HR",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("hr"),
            Some("method 00: identity bypass"),
        ),
        operator(
            "drc",
            "DRC",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("drc"),
            Some("method 00: identity bypass"),
        ),
        operator(
            "cac",
            "CAC",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("cac"),
            Some("method 00: identity bypass"),
        ),
        operator(
            "raw_nr",
            "RAW-NR",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("raw_nr"),
            Some("method 00: identity bypass"),
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
            "dem",
            "DEM",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("dem"),
            None,
        ),
        operator(
            "pfr",
            "PFR",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("pfr"),
            Some("Purple-Fringe Removal; method 00: identity bypass"),
        ),
        operator(
            "color_correction",
            "CCM 8 x 3 x 3",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("color_correction"),
            None,
        ),
        operator(
            "gamma",
            "Gamma",
            "vbe",
            NodeExecutionMode::Enabled,
            Some("gamma"),
            None,
        ),
        operator(
            "three_d_lut",
            "3D LUT 17³",
            "vbe",
            NodeExecutionMode::Bypass,
            Some("three_d_lut"),
            Some("method 00: identity bypass"),
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
        ("dbpc", "out", "sbpc", "in", None),
        ("sbpc", "out", "tintless", "in", None),
        ("tintless", "out", "lsc", "in", None),
        ("lsc", "out", "hr", "in", None),
        ("hr", "out", "drc", "in", None),
        ("drc", "out", "cac", "in", None),
        ("cac", "out", "raw_nr", "in", None),
        ("raw_nr", "out", "wbc", "in", None),
        ("wbc", "out", "dem", "in", None),
        ("dem", "out", "pfr", "in", None),
        ("pfr", "out", "color_correction", "in", None),
        ("color_correction", "out", "gamma", "in", None),
        ("gamma", "out", "three_d_lut", "in", None),
        ("three_d_lut", "out", "rgb2yuv", "in", None),
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
