use std::collections::HashSet;

use rime_core::NodeExecutionMode;
use rime_isp::{build_normal_graph_presentation, build_normal_manifest};

#[test]
fn normal_manifest_contains_the_explicit_main_chain() {
    let manifest = build_normal_manifest();

    assert_eq!(manifest.graph_id, "normal");
    assert_eq!(manifest.graph_kind, "video-isp/normal");
    assert_eq!(manifest.validate(), Ok(()));
    assert_eq!(
        manifest
            .topological_order()
            .expect("normal graph must be acyclic"),
        [
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
        ]
    );
    assert_eq!(manifest.preview_outputs[0].node_id, "rgb2yuv");
    assert_eq!(manifest.preview_outputs.len(), manifest.nodes.len());
    assert_eq!(
        manifest
            .preview_outputs
            .iter()
            .map(|preview| preview.node_id.as_str())
            .collect::<HashSet<_>>(),
        manifest
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<HashSet<_>>(),
    );
}

#[test]
fn blc_is_black_level_correction_and_owns_normalization_contract() {
    let operator = rime_isp::normal_operators()
        .iter()
        .find(|operator| operator.id == "blc")
        .expect("BLC operator");

    assert_eq!(operator.label, "BLC");
    assert_eq!(operator.methods[0].shader_entry, "blc_main");
    assert_eq!(
        operator.methods[0].parameters,
        "black_level white_level width height"
    );
}

#[test]
fn named_operator_outputs_own_rime_q_defaults_without_input_profiles() {
    let expected = [
        ("blc", "s0.14"),
        ("wbc", "s0.12"),
        ("dem", "s0.12"),
        ("rgb2yuv", "s0.10"),
    ];

    for (operator_id, profile) in expected {
        let operator = rime_isp::normal_operators()
            .iter()
            .find(|operator| operator.id == operator_id)
            .expect("named operator");
        assert_eq!(operator.output_rime_q_profile, Some(profile));
    }
}

#[test]
fn generated_normal_quantization_uses_rust_defaults_for_output_modules() {
    let typescript = rime_isp::render_normal_graph_quantization_typescript()
        .expect("normal quantization TypeScript");
    let json = typescript
        .strip_prefix("export const normalGraphQuantization = ")
        .and_then(|value: &str| value.strip_suffix(" as const;\n"))
        .expect("generated quantization export");
    let generated: serde_json::Value = serde_json::from_str(json).expect("generated quantization");

    assert_eq!(generated["graph_id"], "normal");
    assert_eq!(generated["enabled"], true);

    let modules = generated["modules"]
        .as_array()
        .expect("quantization modules");
    let presentation = build_normal_graph_presentation();
    let expected_module_ids = presentation
        .nodes
        .iter()
        .filter_map(|node| node.execution_node_id.as_deref())
        .filter(|module_id| *module_id != "raw_source")
        .collect::<Vec<_>>();
    assert_eq!(
        modules
            .iter()
            .map(|module| module["module_id"].as_str().expect("module id"))
            .collect::<Vec<_>>(),
        expected_module_ids
    );
    assert!(
        modules
            .iter()
            .all(|module| module["module_id"] != "raw_source")
    );

    for (module_id, profile) in [
        ("blc", "s0.14"),
        ("wbc", "s0.12"),
        ("dem", "s0.12"),
        ("rgb2yuv", "s0.10"),
    ] {
        let module = modules
            .iter()
            .find(|module| module["module_id"] == module_id)
            .expect("named quantization module");
        assert_eq!(module["output_profile"], profile);
    }

    for module in modules {
        let module_id = module["module_id"].as_str().expect("module id");
        let node = presentation
            .nodes
            .iter()
            .find(|node| node.execution_node_id.as_deref() == Some(module_id))
            .expect("presentation node");
        assert_eq!(
            module["output_enabled"],
            node.mode == rime_core::NodeExecutionMode::Enabled
        );
    }
    assert!(
        modules
            .iter()
            .all(|module| module.get("dither_enabled").is_none())
    );
    assert!(
        modules
            .iter()
            .all(|module| module.get("input_profile").is_none())
    );
    assert_eq!(
        typescript,
        rime_isp::render_normal_graph_quantization_typescript()
            .expect("deterministic normal quantization TypeScript")
    );
}
#[test]
fn dem_manifest_exposes_methods_and_parameters() {
    let manifest = build_normal_manifest();
    let dem = manifest.node("dem").expect("DEM node");

    assert_eq!(
        dem.methods
            .iter()
            .map(|method| method.method.as_str())
            .collect::<Vec<_>>(),
        ["00", "01", "02", "03", "04"]
    );
    assert_eq!(dem.methods[3].parameters, ["cfa_pattern", "vng_threshold"]);
    assert_eq!(
        dem.methods[4].parameters,
        ["cfa_pattern", "ahd_l_threshold", "ahd_c_threshold_sq"]
    );
}

#[test]
fn pfr_is_separate_from_dem_in_the_manifest() {
    let manifest = build_normal_manifest();
    let dem = manifest.node("dem").expect("DEM node");
    let pfr = manifest.node("pfr").expect("PFR node");

    assert_eq!(dem.outputs[0].domain, rime_core::SignalDomain::LinearRgb);
    assert_eq!(pfr.inputs[0].domain, rime_core::SignalDomain::LinearRgb);
    assert_eq!(pfr.outputs[0].domain, rime_core::SignalDomain::LinearRgb);
    assert!(manifest.node("demosaic").is_none());
    assert!(
        manifest
            .edges
            .iter()
            .any(|edge| edge.from.node_id == "dem" && edge.to.node_id == "pfr")
    );
    assert!(
        manifest
            .edges
            .iter()
            .any(|edge| edge.from.node_id == "pfr" && edge.to.node_id == "color_correction")
    );
}

#[test]
fn presentation_and_manifest_share_executable_nodes() {
    let manifest = build_normal_manifest();
    let presentation = build_normal_graph_presentation();
    let executable: HashSet<&str> = manifest.nodes.iter().map(|node| node.id.as_str()).collect();
    let presented: HashSet<&str> = presentation
        .nodes
        .iter()
        .filter_map(|node| node.execution_node_id.as_deref())
        .collect();

    assert_eq!(presented, executable);
    for id in [
        "sbpc_horizontal",
        "dbpc",
        "sbpc",
        "tintless",
        "lsc",
        "hr",
        "drc",
        "cac",
        "raw_nr",
        "three_d_lut",
    ] {
        assert_eq!(
            presentation
                .node(id)
                .expect("bypass presentation node")
                .mode,
            NodeExecutionMode::Bypass
        );
    }
    assert_eq!(
        presentation.node("pyrd").expect("disabled branch").mode,
        NodeExecutionMode::Disabled
    );
}

#[test]
fn presentation_uses_dem_then_pfr_without_compound_node() {
    let presentation = build_normal_graph_presentation();
    assert_eq!(presentation.node("dem").expect("DEM node").label, "DEM");
    assert_eq!(presentation.node("pfr").expect("PFR node").label, "PFR");
    assert!(presentation.node("demosaic").is_none());
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "wbc" && edge.to == "dem")
    );
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "dem" && edge.to == "pfr")
    );
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "pfr" && edge.to == "color_correction")
    );
}

#[test]
fn presentation_uses_split_vfe_modules_without_legacy_ids() {
    let presentation = build_normal_graph_presentation();

    for (id, label) in [("sbpc", "SBPC"), ("tintless", "TINTLESS"), ("lsc", "LSC")] {
        assert_eq!(
            presentation.node(id).expect("VFE presentation node").label,
            label
        );
    }
    assert!(presentation.node("sbpc_pdpc").is_none());
    assert!(presentation.node("lsc_tintless").is_none());
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "dbpc" && edge.to == "sbpc")
    );
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "sbpc" && edge.to == "tintless")
    );
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "tintless" && edge.to == "lsc")
    );
    assert!(
        presentation
            .edges
            .iter()
            .any(|edge| edge.from == "lsc" && edge.to == "hr")
    );
}

#[test]
fn mctf_instances_share_one_module_with_two_graph_iq_overrides() {
    let presentation = build_normal_graph_presentation();

    assert_eq!(
        presentation
            .iq_overrides
            .iter()
            .map(|override_record| (
                override_record.id.as_str(),
                override_record.module_id.as_str()
            ))
            .collect::<Vec<_>>(),
        [("mctf_1", "mctf"), ("mctf_2", "mctf")]
    );
    for prefix in ["vpe_16", "vpe_4", "vpe_full"] {
        for (position, override_id) in [("mctf_1", "mctf_1"), ("mctf_2", "mctf_2")] {
            let node = presentation
                .node(&format!("{prefix}_{position}"))
                .expect("MCTF instance");
            assert_eq!(node.label, "MCTF");
            assert_eq!(node.module_id.as_deref(), Some("mctf"));
            assert_eq!(node.iq_override_id.as_deref(), Some(override_id));
        }
    }
}

#[test]
fn graph_iq_resolution_uses_override_or_module_default_without_masking_invalid_ids() {
    use rime_core::IqParameterSource;

    let presentation = build_normal_graph_presentation();
    assert_eq!(
        presentation.resolve_iq_source("vpe_full_mctf_1"),
        Ok(Some(IqParameterSource::GraphOverride {
            module_id: "mctf".into(),
            override_id: "mctf_1".into(),
        }))
    );

    let mut defaulted = presentation.clone();
    defaulted
        .nodes
        .iter_mut()
        .find(|node| node.id == "vpe_full_mctf_1")
        .expect("MCTF instance")
        .iq_override_id = None;
    assert_eq!(
        defaulted.resolve_iq_source("vpe_full_mctf_1"),
        Ok(Some(IqParameterSource::ModuleDefault {
            module_id: "mctf".into()
        }))
    );

    let mut invalid = presentation;
    invalid.iq_overrides.clear();
    assert_eq!(
        invalid.resolve_iq_source("vpe_full_mctf_1"),
        Err(rime_core::IqResolutionError::OverrideNotFound {
            node_id: "vpe_full_mctf_1".into(),
            override_id: "mctf_1".into(),
        })
    );
}

#[test]
fn normal_graph_uses_ce_instead_of_color() {
    let presentation = build_normal_graph_presentation();
    assert_eq!(
        presentation.node("vpe_full_ce").expect("CE instance").label,
        "CE"
    );
    assert!(presentation.node("vpe_full_color").is_none());
}

#[test]
fn vpe_uses_ce_between_lce_and_second_mctf() {
    let presentation = build_normal_graph_presentation();
    for prefix in ["vpe_16", "vpe_4", "vpe_full"] {
        let ce = presentation
            .node(&format!("{prefix}_ce"))
            .expect("CE instance");
        assert_eq!(ce.label, "CE");
        assert!(presentation.node(&format!("{prefix}_color")).is_none());
        assert!(
            presentation
                .edges
                .iter()
                .any(|edge| edge.from == format!("{prefix}_lce") && edge.to == ce.id)
        );
        assert!(
            presentation
                .edges
                .iter()
                .any(|edge| edge.from == ce.id && edge.to == format!("{prefix}_mctf_2"))
        );
    }
}
#[test]
fn presentation_omits_non_simulated_vfe_statistics() {
    let presentation = build_normal_graph_presentation();
    let removed = ["ae_awb_st", "afst", "spc", "lrc", "pdst"];

    for id in removed {
        assert!(
            presentation.node(id).is_none(),
            "{id} must not be presented"
        );
        assert!(
            presentation
                .edges
                .iter()
                .all(|edge| edge.from != id && edge.to != id),
            "{id} must not have presentation edges"
        );
    }
}

#[test]
fn presentation_preserves_real_node_and_edge_ports() {
    let presentation = build_normal_graph_presentation();
    let sensor = presentation.node("blc").expect("sensor node");
    let pyrd = presentation.node("pyrd").expect("pyrd node");

    assert_eq!(sensor.inputs, ["in"]);
    assert_eq!(sensor.outputs, ["out"]);
    assert_eq!(pyrd.inputs, ["in"]);
    assert_eq!(pyrd.outputs, ["full", "quarter", "sixteenth"]);

    let pyrd_edges: Vec<_> = presentation
        .edges
        .iter()
        .filter(|edge| edge.from == "pyrd")
        .collect();
    assert_eq!(
        pyrd_edges
            .iter()
            .map(|edge| (
                edge.from_port.as_str(),
                edge.to.as_str(),
                edge.to_port.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("full", "vpe_full_pyrc", "in"),
            ("quarter", "vpe_4_pyrc", "in"),
            ("sixteenth", "vpe_16_pyrc", "in"),
        ]
    );
}

#[test]
fn presentation_uses_manifest_port_order_for_executable_nodes() {
    let manifest = build_normal_manifest();
    let presentation = build_normal_graph_presentation();

    for manifest_node in &manifest.nodes {
        let presented = presentation
            .node(&manifest_node.id)
            .expect("every executable node is presented");
        assert_eq!(
            presented.inputs,
            manifest_node
                .inputs
                .iter()
                .map(|port| port.id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            presented.outputs,
            manifest_node
                .outputs
                .iter()
                .map(|port| port.id.clone())
                .collect::<Vec<_>>()
        );
    }
}
