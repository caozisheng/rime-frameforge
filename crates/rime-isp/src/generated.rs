use rime_core::{Diagnostic, DiagnosticCode};

/// Serializes the Normal Graph manifest as canonical JSON.
///
/// # Errors
///
/// Returns `ManifestInvalid` when serialization fails.
pub fn render_normal_manifest_json() -> Result<String, Diagnostic> {
    serde_json::to_string_pretty(&crate::build_normal_manifest()).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::ManifestInvalid,
            format!("failed to serialize normal manifest: {error}"),
        )
    })
}

/// Renders the generated TypeScript Normal manifest.
///
/// # Errors
///
/// Returns `ManifestInvalid` when manifest serialization fails.
pub fn render_normal_manifest_typescript() -> Result<String, Diagnostic> {
    let json = render_normal_manifest_json()?;
    Ok(format!(
        "export const normalManifest = {json} as const;\n\nexport type NormalManifest = typeof normalManifest;\n"
    ))
}

/// Renders the generated TypeScript Normal Graph presentation.
///
/// # Errors
///
/// Returns `ManifestInvalid` when presentation serialization fails.
pub fn render_normal_graph_presentation_typescript() -> Result<String, Diagnostic> {
    let presentation = crate::build_normal_graph_presentation();
    let graph_json = serde_json::to_string_pretty(&presentation).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::ManifestInvalid,
            format!("failed to serialize normal graph: {error}"),
        )
    })?;
    Ok(format!(
        "export const normalGraphPresentation = {graph_json} as const;\n"
    ))
}

/// Renders the generated TypeScript Normal Graph quantization defaults.
///
/// # Errors
///
/// Returns `ManifestInvalid` when quantization serialization fails.
pub fn render_normal_graph_quantization_typescript() -> Result<String, Diagnostic> {
    let presentation = crate::build_normal_graph_presentation();
    let quantization = rime_core::GraphQuantizationConfig::defaults_for(&presentation).map_err(
        |error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to build normal graph quantization defaults: {error}"),
            )
        },
    )?;
    let quantization_json = serde_json::to_string_pretty(&quantization).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::ManifestInvalid,
            format!("failed to serialize normal graph quantization defaults: {error}"),
        )
    })?;
    Ok(format!(
        "export const normalGraphQuantization = {quantization_json} as const;\n"
    ))
}
