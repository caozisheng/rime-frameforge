use rime_core::{GraphTreeKind, NodeExecutionMode, build_top_graph_presentation};

#[test]
fn top_graph_contains_vfe_vbe_vpe_and_encoder_groups() {
    let graph = build_top_graph_presentation();
    let labels: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == GraphTreeKind::Group
                && (node.id == graph.root_id
                    || node.parent_id.as_deref() == Some(graph.root_id.as_str()))
        })
        .map(|node| node.label.as_str())
        .collect();

    assert_eq!(
        labels,
        [
            "isp pipeline",
            "video front end",
            "video back end",
            "video post",
            "encoder"
        ]
    );
}

#[test]
fn top_graph_compute_nodes_are_enabled() {
    let graph = build_top_graph_presentation();
    let enabled: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == GraphTreeKind::Operator && node.mode == NodeExecutionMode::Enabled
        })
        .map(|node| node.id.as_str())
        .collect();

    assert_eq!(
        enabled,
        [
            "raw_source",
            "blc",
            "wbc",
            "dem",
            "color_correction",
            "gamma",
            "rgb2yuv",
        ]
    );
}

#[test]
fn compatible_unimplemented_image_nodes_are_bypass() {
    let graph = build_top_graph_presentation();
    let sbpc = graph.node("sbpc_horizontal").expect("SBPC node must exist");

    assert_eq!(sbpc.mode, NodeExecutionMode::Bypass);
}

#[test]
fn top_graph_uses_hr_and_same_extent_cac_terminology() {
    let graph = build_top_graph_presentation();
    let hr = graph.node("hr").expect("HR node");
    let cac = graph.node("cac").expect("CAC node");

    assert_eq!(hr.label, "highlight recovery");
    assert_eq!(cac.label, "chromatic aberration correction");
    assert_eq!(cac.mode, NodeExecutionMode::Bypass);
    assert!(graph.node("highlight_recovery").is_none());
    assert!(graph.node("raw_downscale_cac").is_none());
}


#[test]
fn top_graph_separates_static_bad_pixels_color_shading_and_luma_shading() {
    let graph = build_top_graph_presentation();

    assert_eq!(graph.node("sbpc").expect("SBPC node").label, "static bad pixel correction");
    assert_eq!(graph.node("tintless").expect("Tintless node").label, "color shading correction");
    assert_eq!(graph.node("lsc").expect("LSC node").label, "luma shading correction");
    assert!(graph.node("sbpc_pdpc").is_none());
    assert!(graph.node("lsc_tintless").is_none());
}

#[test]
fn graph_without_instance_override_uses_module_default_iq() {
    use rime_core::IqParameterSource;

    let mut graph = build_top_graph_presentation();
    let node = graph.node("video_post").expect("VPE group").clone();
    graph.nodes.push(rime_core::GraphTreeNode {
        id: "mctf_instance".into(),
        label: "MCTF".into(),
        parent_id: Some(node.id),
        kind: GraphTreeKind::Operator,
        mode: NodeExecutionMode::Disabled,
        execution_node_id: None,
        module_id: Some("mctf".into()),
        iq_override_id: None,
        inputs: vec!["in".into()],
        outputs: vec!["out".into()],
        reason: None,
        default_expanded: false,
    });

    assert_eq!(
        graph.resolve_iq_source("mctf_instance"),
        Ok(Some(IqParameterSource::ModuleDefault { module_id: "mctf".into() }))
    );
}
#[test]
fn unsupported_statistics_are_omitted_and_pyramid_branches_are_disabled() {
    let graph = build_top_graph_presentation();

    for id in ["statistics", "ae_awb_stats", "af_stats", "pdaf_stats"] {
        assert!(graph.node(id).is_none(), "{id} must not be presented");
    }
    assert_eq!(
        (
            graph.node("yuv_pyramid").expect("pyramid node").mode,
            graph.node("video_post").expect("VPE group").mode,
        ),
        (NodeExecutionMode::Disabled, NodeExecutionMode::Disabled)
    );
}

#[test]
fn unavailable_output_paths_are_branches_not_operators() {
    let graph = build_top_graph_presentation();

    for id in ["yuv_pyramid", "vpe_full"] {
        assert_eq!(
            graph.node(id).expect("branch node").kind,
            GraphTreeKind::Branch
        );
    }
}

#[test]
fn active_groups_are_expanded_and_disabled_groups_are_collapsed() {
    let graph = build_top_graph_presentation();

    assert!(graph.node("video_front_end").expect("VFE").default_expanded);
    assert!(graph.node("video_back_end").expect("VBE").default_expanded);
    assert!(!graph.node("video_post").expect("VPE").default_expanded);
    assert!(!graph.node("encoder").expect("encoder").default_expanded);
}
