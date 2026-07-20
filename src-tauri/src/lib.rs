use tauri::Manager;

mod base_plugin;
mod chunker;
mod commands;
pub mod config;
mod epub;
mod merger;
mod model_manager;
pub mod plugin_manager;
pub mod plugins;
mod recovery;
pub mod sidecars;
mod utils;
mod wizard;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let path_resolver = app.path();
            let app_data_dir = match path_resolver.app_data_dir() {
                Ok(p) => {
                    eprintln!("[setup] app_data_dir (from tauri): {}", p.display());
                    p
                }
                Err(e) => {
                    eprintln!("[setup] tauri app_data_dir failed: {e}; falling back");
                    let fallback = std::env::var("LOCALAPPDATA")
                        .map(|p| std::path::PathBuf::from(p).join("com.patata.audiobookgenerator"))
                        .or_else(|_| {
                            std::env::var("HOME").map(|p| {
                                std::path::PathBuf::from(p)
                                    .join(".local/share/com.patata.audiobookgenerator")
                            })
                        })
                        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| ".".into()));
                    eprintln!("[setup] fallback app_data_dir: {}", fallback.display());
                    fallback
                }
            };
            if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                eprintln!(
                    "[setup] could not create app data dir {}: {e}",
                    app_data_dir.display()
                );
            }
            config::paths::set_app_data_dir(app_data_dir.clone());
            let pm = Arc::new(plugin_manager::PluginManager::new(app_data_dir));
            eprintln!(
                "[setup] plugin manager ready: {} engine(s) registered",
                pm.catalogue().len()
            );
            app.manage(pm);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine_status,
            commands::engine_defaults,
            commands::load_engine,
            commands::unload_engine,
            commands::stop_generation,
            commands::load_epub,
            commands::check_recovery,
            commands::scan_recovery_books,
            commands::get_failed_chunks,
            commands::retry_failed_chunks,
            commands::split_and_retry_chunk,
            commands::merge_chapter_chunks,
            commands::synthesize_demo,
            commands::start_generation,
            commands::get_test_epub,
            commands::list_mp3s_in_dir,
            commands::get_default_output_dir,
            commands::list_models,
            commands::is_model_installed,
            commands::download_model,
            commands::remove_model,
            commands::get_models_path,
            wizard::detect_hardware,
            wizard::check_dependencies,
            wizard::get_wizard_steps,
            wizard::is_wizard_done,
            wizard::mark_wizard_done,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}