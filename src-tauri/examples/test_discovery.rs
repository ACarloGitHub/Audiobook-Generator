// Direct test of engine discovery — bypasses the Tauri runtime to
// print exactly what `EngineRegistry::new()` sees on disk.

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("=== Engine discovery diagnostic ===");
    let app_data = std::env::var("ABG_APP_DATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            // Default to the per-user app data dir that Tauri uses
            // on Windows: `%LOCALAPPDATA%\AudiobookGenerator\`.
            std::env::var("LOCALAPPDATA")
                .map(|p| std::path::PathBuf::from(p).join("AudiobookGenerator"))
                .unwrap_or_else(|_| {
                    std::env::current_dir()
                        .unwrap()
                        .join("target")
                        .join("app-data")
                })
        });
    std::fs::create_dir_all(&app_data).ok();
    let registry = audiobook_generator_lib::engines::EngineRegistry::new(app_data.clone());
    println!("App data dir: {}", app_data.display());
    println!("\nRegistered (real): {} engines", registry.list().len());
    for e in registry.list() {
        println!("  - id={} display={} installed={}", e.id, e.display_name, e.installed);
    }
    println!("\nCatalogue (registered + stubs): {} entries", registry.catalogue().len());
    for e in registry.catalogue() {
        println!("  - id={} display={} installed={}", e.id, e.display_name, e.installed);
    }
    Ok(())
}