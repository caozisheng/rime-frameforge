use crate::{Diagnostic, DiagnosticCode};

/// Renders the complete Top Graph presentation consumed by the React tree.
///
/// # Errors
///
/// Returns `ManifestInvalid` if the presentation cannot be serialized.
pub fn render_top_graph_presentation_typescript() -> Result<String, Diagnostic> {
    let json =
        serde_json::to_string_pretty(&crate::build_top_graph_presentation()).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to serialize graph presentation: {error}"),
            )
        })?;
    Ok(format!(
        "export const topGraphPresentation = {json} as const;\n"
    ))
}
