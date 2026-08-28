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
    let json = serde_json::to_string_pretty(&crate::build_normal_graph_presentation()).map_err(
        |error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to serialize normal graph: {error}"),
            )
        },
    )?;
    Ok(format!(
        "export const normalGraphPresentation = {json} as const;\n"
    ))
}
