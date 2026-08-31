#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod dng_command;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            dng_command::list_dng_sequence,
            dng_command::inspect_dng_frame,
            dng_command::read_dng_raw,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("failed to run Rime desktop: {error}"));
}
