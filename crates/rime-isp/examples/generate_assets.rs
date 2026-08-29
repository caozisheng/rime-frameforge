use std::{fs, path::Path};

use rime_core::render_top_graph_presentation_typescript;

use rime_isp::{
    render_normal_graph_presentation_typescript, render_normal_graph_quantization_typescript,
    render_normal_manifest_typescript,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root not found")?;
    let web_dir = root.join("web/src/generated");
    fs::create_dir_all(&web_dir)?;
    fs::write(
        web_dir.join("normal_manifest.generated.ts"),
        render_normal_manifest_typescript()?,
    )?;
    fs::write(
        web_dir.join("top_graph.generated.ts"),
        render_top_graph_presentation_typescript()?,
    )?;
    fs::write(
        web_dir.join("normal_graph.generated.ts"),
        render_normal_graph_presentation_typescript()?,
    )?;
    fs::write(
        web_dir.join("normal_quantization.generated.ts"),
        render_normal_graph_quantization_typescript()?,
    )?;
    Ok(())
}
