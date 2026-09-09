use rime_isp::operator_by_id;

/// WGSL validity check via naga: the CR operator shader must parse and
/// validate, including its storage buffer declarations.
#[test]
fn color_reproduce_wgsl_validates_in_naga() {
    let operator = operator_by_id("color_reproduce").expect("CR operator");
    let manifest = operator.definition();
    let method = manifest
        .methods
        .iter()
        .find(|method| method.method == manifest.default_method)
        .expect("default method");
    let shader = operator.shader("00").expect("shader asset");
    let module = naga::front::wgsl::parse_str(shader.source).expect("WGSL must parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("WGSL must validate: {error}"));
    let _ = info;
    assert_eq!(shader.entry_point, method.shader_entry);
}
