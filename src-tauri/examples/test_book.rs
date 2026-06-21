// Integration test: load Kokoro, run book-level synthesis.

use audiobook_generator_lib::engines::Engine;
use audiobook_generator_lib::engines::kokoro::{synthesize_book, KokoroEngine, KokoroPaths};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("project root")
        .to_path_buf();

    let paths = KokoroPaths {
        model_dir: project_root.join("src-tauri/models/kokoro/models"),
        voices_dir: project_root.join("src-tauri/models/kokoro/voices"),
    };

    let book_path = project_root.join("src-tauri/test_books/perrault.epub");
    let out_dir = project_root.join("target/test-book-out");
    std::fs::create_dir_all(&out_dir)?;

    let ffmpeg = audiobook_generator_lib::merger::find_ffmpeg()
        .map_err(|e| anyhow::anyhow!("ffmpeg not on PATH: {e}"))?;
    println!("ffmpeg: {}", ffmpeg.display());

    println!("\n== loading Kokoro ==");
    let engine = KokoroEngine::new(paths, "af_heart");
    let t0 = std::time::Instant::now();
    let handle = engine.load("kokoro-82M-quantized")
        .map_err(|e| anyhow::anyhow!("load failed: {e:#}"))?;
    println!("loaded in {:?}", t0.elapsed());

    println!("\n== synthesizing book ==");
    let t1 = std::time::Instant::now();
    let chapters = synthesize_book(
        &engine,
        &handle,
        &book_path,
        &out_dir,
        6,
        &ffmpeg,
        None,
    )
    .map_err(|e| anyhow::anyhow!("synthesize_book failed: {e:#}"))?;
    println!("synthesized {} chapters in {:?}", chapters, t1.elapsed());

    engine.unload(&handle)?;

    let mp3s: Vec<_> = std::fs::read_dir(&out_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mp3".into()))
        .collect();
    println!("\nMP3 files in output dir: {}", mp3s.len());
    for m in mp3s.iter().take(5) {
        let meta = std::fs::metadata(m.path())?;
        println!("  {} ({:.1} KB)", m.path().display(), meta.len() as f64 / 1024.0);
    }

    Ok(())
}