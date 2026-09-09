use std::collections::HashSet;

use rime_isp::{OperatorDefinition, normal_operators};

#[test]
fn normal_operators_use_two_digit_methods_with_shared_io_contracts() {
    for operator in normal_operators() {
        assert_operator_methods_are_valid(operator.definition());
    }
}

#[test]
fn every_method_manifest_owns_shader_and_lifecycle_hooks() {
    for operator in normal_operators() {
        for method in operator.definition().methods {
            assert_eq!(method.shader.method, method.method);
            assert!(!method.shader.source.is_empty());
        }
    }
}
#[test]
fn normal_graph_registers_every_explicit_main_chain_operator() {
    let ids: HashSet<&str> = normal_operators()
        .iter()
        .map(|operator| operator.definition().id)
        .collect();

    assert_eq!(
        ids,
        HashSet::from([
            "blc",
            "sbpc_horizontal",
            "dbpc",
            "sbpc",
            "raw_nr",
            "tintless",
            "lsc",
            "wbc",
            "cac",
            "drc",
            "dem",
            "pfr",
            "color_reproduce",
            "gamma",
            "three_d_lut",
            "rgb2yuv",
        ])
    );
}

#[test]
fn wbc_owns_highlight_recovery_and_cac_uses_industry_name() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.definition().id, operator.definition()))
        .collect();
    let wbc = operators.get("wbc").expect("WBC operator");
    let cac = operators.get("cac").expect("CAC operator");

    assert_eq!(wbc.label, "WBC");
    assert!(
        wbc.methods[0]
            .parameters
            .split_whitespace()
            .any(|parameter| parameter == "highlight_recovery")
    );
    assert_eq!(cac.label, "CAC");
    let cac_method = cac.methods.first().expect("CAC method");
    assert_eq!(cac_method.input, cac_method.output);
    assert!(!operators.contains_key("hr"));
    assert!(!operators.contains_key("hlr"));
    assert!(!operators.contains_key("raw_ds_cac"));
}

#[test]
fn vfe_shading_operators_have_separate_same_extent_contracts() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.definition().id, operator.definition()))
        .collect();

    for (id, label) in [("sbpc", "SBPC"), ("tintless", "TINTLESS"), ("lsc", "LSC")] {
        let operator = operators.get(id).expect("VFE operator");
        assert_eq!(operator.label, label);
        let method = operator.methods.first().expect("VFE method");
        assert_eq!(method.input, method.output);
    }
    assert!(!operators.contains_key("sbpc_pdpc"));
    assert!(!operators.contains_key("lsc_tintless"));
}

#[test]
fn mctf_uses_one_module_schema_and_default_iq_table() {
    assert_eq!(rime_isp::vpe::mctf::METHOD_00, "00");
    assert_eq!(rime_isp::vpe::mctf::PARAMETER_SCHEMA_ID, "mctf:00:v1");
    assert_eq!(rime_isp::vpe::mctf::DEFAULT_IQ_TABLE_ID, "mctf:00:default");
}

#[test]
fn ce_replaces_color_as_the_vpe_operator_name() {
    assert!(
        !normal_operators()
            .iter()
            .any(|operator| operator.definition().id == "color")
    );
    assert_eq!(rime_isp::vpe::ce::METHOD_00, "00");
}
#[test]
fn dem_and_pfr_have_separate_operator_contracts() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.definition().id, operator.definition()))
        .collect();
    let dem = operators.get("dem").expect("DEM operator");
    let pfr = operators.get("pfr").expect("PFR operator");
    assert_eq!(dem.label, "DEM");
    assert_eq!(dem.methods.len(), 5);
    assert_eq!(pfr.label, "PFR");
    let pfr_method = pfr.methods.first().expect("PFR method");
    assert_eq!(pfr_method.input.domain, rime_core::SignalDomain::LinearRgb);
    assert_eq!(pfr_method.output, pfr_method.input);
    assert!(!operators.contains_key("demosaic"));
}

#[test]
fn wbc_shader_indexes_rgb_gains_by_cfa_channel() {
    let shader = include_str!("../src/vfe/white_balance/wbc00.wgsl");

    assert!(shader.contains("gains: vec4<f32>"));
    assert!(shader.contains("params.cfa_pattern"));
    assert!(shader.contains("params.gains[channel]"));
    assert!(shader.contains("highlight_recovery"));
    assert!(!shader.contains("gain = 2.0"));
    assert!(!shader.contains("gain = 1.5"));
}

#[test]
fn gamma_exposes_adjustable_exponent_and_luminance_lut() {
    let gamma = normal_operators()
        .iter()
        .find(|operator| operator.definition().id == "gamma")
        .expect("Gamma operator")
        .definition();
    let method = gamma.methods.first().expect("Gamma method");
    assert_eq!(method.parameters, "gamma gamma_lut");
    assert_eq!(method.shader.bindings.uniform, Some(2));
}

#[test]
fn dem_registers_reference_methods_and_cfa_parameters() {
    let dem = normal_operators()
        .iter()
        .find(|operator| operator.definition().id == "dem")
        .expect("DEM operator")
        .definition();
    assert_eq!(
        dem.methods
            .iter()
            .map(|method| method.method)
            .collect::<Vec<_>>(),
        ["00", "01", "02", "03", "04"]
    );
    for method in dem.methods {
        assert!(method.parameters.contains("cfa"));
        assert!(method.shader_entry.starts_with("demosaic_"));
    }
}

#[test]
fn drc_registers_global_and_local_tone_methods_with_shared_raw_contracts() {
    let drc = normal_operators()
        .iter()
        .find(|operator| operator.definition().id == "drc")
        .expect("DRC operator")
        .definition();
    assert_eq!(drc.mode, rime_core::NodeExecutionMode::Enabled);
    assert_eq!(
        drc.methods
            .iter()
            .map(|method| method.method)
            .collect::<Vec<_>>(),
        ["00", "01"]
    );
    let global = &drc.methods[0];
    let local = &drc.methods[1];
    assert_eq!(global.input, local.input);
    assert_eq!(global.output, local.output);
    assert_eq!(global.input, global.output);
    assert_eq!(global.shader.bindings.uniform, Some(0));
    assert_eq!(local.shader.bindings.uniform, Some(0));
    assert!(global.parameters.contains("global_tone_lut"));
    assert!(local.parameters.contains("local_tone_lut"));
    assert_eq!(global.shader.source, local.shader.source);
    assert_eq!(global.shader.stages.len(), 6);
    assert_eq!(local.shader.stages.len(), 6);
    let global_combine = global
        .shader
        .stages
        .iter()
        .find(|stage| stage.entry_point == "drc_combine_global_main")
        .expect("global combine stage");
    assert!(
        global_combine
            .bindings
            .iter()
            .any(|binding| binding.binding == 6)
    );
    let local_combine = local
        .shader
        .stages
        .iter()
        .find(|stage| stage.entry_point == "drc_combine_local_main")
        .expect("local combine stage");
    assert!(
        local_combine
            .bindings
            .iter()
            .any(|binding| binding.binding == 7)
    );
    assert_ne!(global.shader.entry_point, local.shader.entry_point);
}

#[test]
fn every_demosaic_method_uses_shared_rgb_saturation_clipping() {
    let dem = normal_operators()
        .iter()
        .find(|operator| operator.definition().id == "dem")
        .expect("DEM operator")
        .definition();
    for method in dem.methods {
        assert!(
            method.shader.source.contains("shared_saturation_clip"),
            "DEM method {} must preserve RGB ratios at saturation",
            method.method
        );
    }
}

#[test]
fn color_reproduce_applies_two_matrices_and_nearest_neighbor_hs_lut() {
    let cr = normal_operators()
        .iter()
        .find(|operator| operator.definition().id == "color_reproduce")
        .expect("color reproduce operator")
        .definition();
    let method = &cr.methods[0];
    assert_eq!(method.shader_entry, "color_reproduce_main");
    assert_eq!(
        method.parameters,
        "sensor_to_prophoto hs_lut prophoto_to_srgb"
    );
    let source = method.shader.source;
    // Two matrix applications (sensor->ProPhoto, ProPhoto->sRGB).
    assert!(
        source.contains("apply_matrix(0u"),
        "sensor->ProPhoto matrix"
    );
    assert!(source.contains("apply_matrix(1u"), "ProPhoto->sRGB matrix");
    // Nearest-neighbour HS LUT (ValueDivs == 1) in DNG grid order, gated
    // by hs_enable.
    assert!(
        source.contains("cr_hs_lut.values[3u * entry"),
        "nearest-neighbour LUT"
    );
    assert!(
        source.contains("dims_and_enable.z == 0u"),
        "HS disable gate"
    );
    assert!(
        !source.contains("* hue_divs + h) * sat_divs + s"),
            "value dimension must not participate in the lookup index"
    );
    // Three independent per-channel clips (MATLAB steps 7, 10, 12); the
    // ratio-preserving saturation clip belongs to the DEM family, not CR.
    assert!(!source.contains("shared_saturation_clip"));
}

fn assert_operator_methods_are_valid(operator: &OperatorDefinition) {
    assert!(
        !operator.methods.is_empty(),
        "{} has no methods",
        operator.id
    );
    let mut method_ids = HashSet::new();
    for method in operator.methods {
        assert!(
            method.method.len() == 2 && method.method.bytes().all(|byte| byte.is_ascii_digit()),
            "{} has invalid method {}",
            operator.id,
            method.method
        );
        assert!(
            method_ids.insert(method.method),
            "duplicate method {}",
            method.method
        );
        assert_eq!(
            method.input,
            operator.methods.first().expect("default method").input,
            "{} input differs",
            operator.id
        );
        assert_eq!(
            method.output,
            operator.methods.first().expect("default method").output,
            "{} output differs",
            operator.id
        );
        assert!(!method.shader_entry.is_empty());
        assert!(!method.parameters.is_empty());
    }
}
