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

    assert_eq!(
        graph.node("sbpc").expect("SBPC node").label,
        "static bad pixel correction"
    );
    assert_eq!(
        graph.node("tintless").expect("Tintless node").label,
        "color shading correction"
    );
    assert_eq!(
        graph.node("lsc").expect("LSC node").label,
        "luma shading correction"
    );
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
        Ok(Some(IqParameterSource::ModuleDefault {
            module_id: "mctf".into()
        }))
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

#[test]
fn graph_quantization_defaults_exclude_raw_source() {
    use rime_core::GraphQuantizationConfig;

    let graph = build_top_graph_presentation();
    let config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");

    assert!(config.module("raw_source").is_none());
    assert_eq!(config.module("blc").unwrap().output_profile, "u0.14");
    assert_eq!(config.module("wbc").unwrap().output_profile, "u0.12");
    assert_eq!(config.module("dem").unwrap().output_profile, "u0.12");
    assert_eq!(config.module("rgb2yuv").unwrap().output_profile, "u0.10");
}

#[test]
fn graph_quantization_rejects_graph_id_mismatch() {
    use rime_core::{GraphQuantizationConfig, GraphQuantizationError};

    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.graph_id = "other".into();

    assert!(matches!(
        config.resolve(&graph),
        Err(GraphQuantizationError::GraphIdMismatch { .. })
    ));
}

#[test]
fn graph_quantization_rejects_missing_module_preference() {
    use rime_core::{GraphQuantizationConfig, GraphQuantizationError};

    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.modules.retain(|module| module.module_id != "dem");

    assert!(matches!(
        config.resolve(&graph),
        Err(GraphQuantizationError::MissingModule { module_id }) if module_id == "dem"
    ));
}

#[test]
fn graph_quantization_serializes_output_only_preferences_and_derives_mode() {
    use rime_core::GraphQuantizationConfig;

    let graph = build_top_graph_presentation();
    let config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    let json = serde_json::to_value(&config).expect("serialize");
    let module = &json["modules"][0];
    assert!(module.get("mode").is_none());
    assert!(module.get("dither_enabled").is_none());
    let state = config.resolve(&graph).expect("resolve");
    assert_eq!(
        state.module("blc").unwrap().mode,
        NodeExecutionMode::Enabled
    );
}

#[test]
fn graph_quantization_derives_dither_from_effective_output_and_clip_type() {
    use rime_core::GraphQuantizationConfig;
    use rime_quant::ClipType;

    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.module_mut("blc").unwrap().clip_type = ClipType::Dither;

    let state = config.resolve(&graph).expect("resolve");
    let blc = state.module("blc").expect("BLC state");
    assert!(blc.effective_output_enabled);
    assert!(blc.effective_dither_enabled);

    config.module_mut("blc").unwrap().output_enabled = false;
    let state = config.resolve(&graph).expect("resolve");
    let blc = state.module("blc").expect("BLC state");
    assert!(!blc.effective_output_enabled);
    assert!(!blc.effective_dither_enabled);

    config.module_mut("blc").unwrap().output_enabled = true;
    config.module_mut("blc").unwrap().clip_type = ClipType::Truncate;
    let state = config.resolve(&graph).expect("resolve");
    assert!(!state.module("blc").expect("BLC state").effective_dither_enabled);
}

#[test]
fn graph_quantization_global_off_forces_effective_outputs_off() {
    use rime_core::GraphQuantizationConfig;
    use rime_quant::ClipType;

    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.module_mut("blc").unwrap().clip_type = ClipType::Dither;
    config.enabled = false;
    let state = config.resolve(&graph).expect("resolve");

    let blc = state.module("blc").expect("BLC state");
    assert!(!blc.effective_output_enabled);
    assert!(!blc.effective_dither_enabled);
    assert!(blc.preference.output_enabled);
}

#[test]
fn graph_quantization_disabled_and_bypass_modes_force_outputs_off() {
    use rime_core::GraphQuantizationConfig;
    let mut graph = build_top_graph_presentation();
    graph.nodes.push(rime_core::GraphTreeNode {
        id: "disabled_module".into(),
        label: "disabled".into(),
        parent_id: None,
        kind: GraphTreeKind::Operator,
        mode: NodeExecutionMode::Disabled,
        execution_node_id: Some("disabled_module".into()),
        module_id: None,
        iq_override_id: None,
        inputs: vec!["in".into()],
        outputs: vec!["out".into()],
        reason: None,
        default_expanded: false,
    });
    graph.nodes.push(rime_core::GraphTreeNode {
        id: "bypass_module".into(),
        label: "bypass".into(),
        parent_id: None,
        kind: GraphTreeKind::Operator,
        mode: NodeExecutionMode::Bypass,
        execution_node_id: Some("bypass_module".into()),
        module_id: None,
        iq_override_id: None,
        inputs: vec!["in".into()],
        outputs: vec!["out".into()],
        reason: None,
        default_expanded: false,
    });
    let config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    let state = config.resolve(&graph).expect("resolve");

    assert!(
        !state
            .module("disabled_module")
            .expect("disabled module")
            .effective_output_enabled
    );
    assert!(
        !state
            .module("bypass_module")
            .expect("bypass module")
            .effective_output_enabled
    );
    assert_eq!(
        state.module("bypass_module").unwrap().mode,
        NodeExecutionMode::Bypass
    );
}

#[test]
fn graph_quantization_reopening_global_switch_restores_preferences() {
    use rime_core::GraphQuantizationConfig;
    use rime_quant::ClipType;

    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.module_mut("blc").unwrap().clip_type = ClipType::Dither;
    config.enabled = false;
    assert!(
        !config
            .resolve(&graph)
            .unwrap()
            .module("blc")
            .unwrap()
            .effective_dither_enabled
    );
    config.enabled = true;
    let state = config.resolve(&graph).expect("resolve");
    assert!(state.module("blc").unwrap().effective_output_enabled);
    assert!(state.module("blc").unwrap().effective_dither_enabled);
}

#[test]
fn graph_quantization_rejects_unknown_modules_and_malformed_profiles() {
    use rime_core::{GraphQuantizationConfig, GraphQuantizationError};
    let graph = build_top_graph_presentation();
    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.modules[0].module_id = "missing".into();
    assert!(matches!(
        config.resolve(&graph),
        Err(GraphQuantizationError::UnknownModule { .. })
    ));

    let mut config = GraphQuantizationConfig::defaults_for(&graph).expect("defaults");
    config.module_mut("blc").unwrap().output_profile = "u25.1".into();
    assert!(matches!(
        config.resolve(&graph),
        Err(GraphQuantizationError::InvalidProfile { .. })
    ));
}
