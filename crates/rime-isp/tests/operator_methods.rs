use std::collections::HashSet;

use rime_isp::{normal_operators, OperatorDefinition};

#[test]
fn normal_operators_use_two_digit_methods_with_shared_io_contracts() {
    for operator in normal_operators() {
        assert_operator_methods_are_valid(operator);
    }
}

#[test]
fn normal_graph_registers_every_explicit_main_chain_operator() {
    let ids: HashSet<&str> = normal_operators()
        .iter()
        .map(|operator| operator.id)
        .collect();

    assert_eq!(
        ids,
        HashSet::from([
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
        ])
    );
}

#[test]
fn hr_and_cac_use_industry_names_and_same_extent_bayer_contracts() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.id, *operator))
        .collect();
    let hr = operators.get("hr").expect("HR operator");
    let cac = operators.get("cac").expect("CAC operator");

    assert_eq!(hr.label, "HR");
    assert_eq!(cac.label, "CAC");
    assert_eq!(cac.input, cac.output);
    assert!(!operators.contains_key("hlr"));
    assert!(!operators.contains_key("raw_ds_cac"));
}

#[test]
fn vfe_shading_operators_have_separate_same_extent_contracts() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.id, *operator))
        .collect();

    for (id, label) in [("sbpc", "SBPC"), ("tintless", "TINTLESS"), ("lsc", "LSC")] {
        let operator = operators.get(id).expect("VFE operator");
        assert_eq!(operator.label, label);
        assert_eq!(operator.input, operator.output);
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
    assert!(!normal_operators().iter().any(|operator| operator.id == "color"));
    assert!(rime_isp::vpe::ce::METHOD_00 == "00");
}
#[test]
fn dem_and_pfr_have_separate_operator_contracts() {
    let operators: std::collections::HashMap<_, _> = normal_operators()
        .iter()
        .map(|operator| (operator.id, *operator))
        .collect();
    let dem = operators.get("dem").expect("DEM operator");
    let pfr = operators.get("pfr").expect("PFR operator");

    assert_eq!(dem.label, "DEM");
    assert_eq!(dem.methods.len(), 5);
    assert_eq!(pfr.label, "PFR");
    assert_eq!(pfr.input.domain, rime_core::SignalDomain::LinearRgb);
    assert_eq!(pfr.output, pfr.input);
    assert!(!operators.contains_key("demosaic"));
}

#[test]
fn dem_registers_reference_methods_and_cfa_parameters() {
    let dem = normal_operators()
        .iter()
        .find(|operator| operator.id == "dem")
        .expect("DEM operator");
    assert_eq!(
        dem.methods.iter().map(|method| method.method).collect::<Vec<_>>(),
        ["00", "01", "02", "03", "04"]
    );
    for method in dem.methods {
        assert!(method.parameters.contains("cfa"));
        assert!(method.shader_entry.starts_with("demosaic_"));
    }
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
            method.input, operator.input,
            "{} input differs",
            operator.id
        );
        assert_eq!(
            method.output, operator.output,
            "{} output differs",
            operator.id
        );
        assert!(!method.shader_entry.is_empty());
        assert!(!method.parameters.is_empty());
    }
}
