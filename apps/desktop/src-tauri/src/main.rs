#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod dng_command;
mod native_pipeline;

fn main() {
    tauri::Builder::default()
        .manage(native_pipeline::NativePipelineState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            dng_command::list_dng_sequence,
            dng_command::read_dng_frame,
            native_pipeline::inspect_dng_native,
            native_pipeline::render_dng_native,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("failed to run Rime desktop: {error}"));
}
