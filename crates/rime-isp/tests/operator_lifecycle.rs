use rime_isp::{normal_operators, operator_by_id};

#[test]
fn normal_registry_exposes_complete_executable_operator_assets() {
    let operators = normal_operators();

    assert_eq!(operators.len(), 17);
    for operator in operators {
        let definition = operator.definition();
        let shader = operator
            .shader(definition.default_method)
            .expect("default method must own a shader asset");

        assert_eq!(
            shader.entry_point,
            definition
                .methods
                .iter()
                .find(|method| method.method == definition.default_method)
                .expect("default method")
                .shader_entry
        );
        assert_ne!(shader.bindings.input, shader.bindings.output);
        assert_eq!(
            shader.bindings.uniform.is_some(),
            matches!(definition.id, "blc" | "wbc" | "dem" | "gamma"),
            "{} uniform binding differs from its parameter packet contract",
            definition.id
        );
    }
}

#[test]
fn operator_lookup_returns_the_registered_trait_object() {
    let wbc = operator_by_id("wbc").expect("WBC operator");

    assert_eq!(wbc.definition().label, "WBC");
    assert!(operator_by_id("pyrd").is_none());
}
