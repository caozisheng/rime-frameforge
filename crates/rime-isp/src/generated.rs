use std::fmt::Write as _;

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
    let quantization =
        rime_core::GraphQuantizationConfig::defaults_for(&presentation).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to build normal graph quantization defaults: {error}"),
            )
        })?;
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

/// Renders the shared DRC multi-pass WGSL as a TypeScript string asset.
///
/// # Errors
///
/// Returns `ManifestInvalid` when the WGSL string cannot be serialized.
pub fn render_drc_pipeline_typescript() -> Result<String, Diagnostic> {
    let source = serde_json::to_string(crate::vbe::drc::DRC_PIPELINE_WGSL).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::ManifestInvalid,
            format!("failed to serialize DRC pipeline WGSL: {error}"),
        )
    })?;
    Ok(format!("export const drcPipelineWgsl = {source};\n"))
}

/// Renders the WBC single-source WGSL as a TypeScript string asset.
///
/// # Errors
///
/// Returns `ManifestInvalid` when the WGSL string cannot be serialized.
pub fn render_wbc_pipeline_typescript() -> Result<String, Diagnostic> {
    let source =
        serde_json::to_string(crate::vfe::white_balance::WBC_PIPELINE_WGSL).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to serialize WBC pipeline WGSL: {error}"),
            )
        })?;
    Ok(format!("export const wbcPipelineWgsl = {source};\n"))
}

/// Renders the fused-view Normal Graph WGSL as a TypeScript string asset.
///
/// # Errors
///
/// Returns `ManifestInvalid` when the WGSL string cannot be serialized.
pub fn render_fused_pipeline_typescript() -> Result<String, Diagnostic> {
    let source = serde_json::to_string(&crate::fused_view::render_fused_normal_shader()).map_err(
        |error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to serialize fused pipeline WGSL: {error}"),
            )
        },
    )?;
    Ok(format!("export const fusedPipelineWgsl = {source};\n"))
}

/// Renders the segmented (complex-DEM) Normal Graph shader set as one asset.
///
/// # Errors
///
/// Returns `ManifestInvalid` when a shader cannot be serialized or the DEM
/// method is unknown.
pub fn render_segmented_fused_typescript() -> Result<String, Diagnostic> {
    let mut segments = String::new();
    for method in ["01", "02", "03", "04"] {
        let shaders =
            crate::fused_view::render_segmented_normal_shaders(method).map_err(|error| {
                Diagnostic::new(
                    DiagnosticCode::ManifestInvalid,
                    format!("failed to render segmented Normal Graph shaders: {error}"),
                )
            })?;
        let [pre, dem, quantize, post] = shaders;
        let json = serde_json::json!({
            "pre": pre,
            "dem": dem,
            "quantize": quantize,
            "post": post,
        });
        let serialized = serde_json::to_string(&json).map_err(|error| {
            Diagnostic::new(
                DiagnosticCode::ManifestInvalid,
                format!("failed to serialize segmented Normal Graph shaders: {error}"),
            )
        })?;
        let _ = writeln!(
            segments,
            "export const segmented{method}Shaders = {serialized};"
        );
    }
    Ok(segments)
}

/// Renders the BLC single-source WGSL as a TypeScript string asset.
///
/// # Errors
///
/// Returns `ManifestInvalid` when the WGSL string cannot be serialized.
pub fn render_blc_pipeline_typescript() -> Result<String, Diagnostic> {
    let source = serde_json::to_string(crate::vfe::blc::BLC_PIPELINE_WGSL).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::ManifestInvalid,
            format!("failed to serialize BLC pipeline WGSL: {error}"),
        )
    })?;
    Ok(format!("export const blcPipelineWgsl = {source};\n"))
}
