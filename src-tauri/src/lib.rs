//! Audiobook Generator — Tauri application entry point.
//!
//! The Tauri commands defined here are intentionally engine-agnostic. Each
//! engine (Kokoro, Qwen3-TTS, OuteTTS, NeuTTS Air) implements the
//! `Engine` trait. The frontend talks only to the commands in this module
//! and to the engine registry.
//!
//! See AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md
//! and AudiobookGenerator-Wiki/wiki/concepts/engine-lifecycle.md.

mod commands;
mod engines;
mod recovery;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .manage(engines::EngineRegistry::new())
        .invoke_handler(tauri::generate_handler![
            commands::engine_status,
            commands::load_engine,
            commands::unload_engine,
            commands::synthesize,
            commands::stop_generation,
            commands::check_recovery,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
