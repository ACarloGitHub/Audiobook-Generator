// Integration test: load the Kokoro engine, synthesize a test sentence,
// verify the WAV is produced. Run with:
//
//     cargo run --bin test_kokoro
//
// This is a smoke test for the engine; the real Tauri UI is exercised
// by `cargo tauri dev` / `cargo tauri build`.

use audiobook_generator_lib::engines::kokoro::{synthesize_book, KokoroEngine, KokoroPaths};
use audiobook_generator_lib::engines::{Engine, EngineHandle};
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
    println!("Kokoro model dir:  {}", paths.model_dir.display());
    println!("Kokoro voices dir: {}", paths.voices_dir.display());

    let engine = KokoroEngine::new(paths.clone(), "af_heart");
    let handle = engine
        .load("kokoro-82M-quantized")
        .map_err(|e| anyhow::anyhow!("load failed: {e:#}"))?;
    println!("Loaded engine: {:?}", handle);

    let out_dir = project_root.join("target/test-out");
    std::fs::create_dir_all(&out_dir)?;
    let wav = out_dir.join("demo.wav");
    let request = audiobook_generator_lib::engines::SynthesizeRequest {
        text: "Hello, this is a test of the Audiobook Generator native shell with Kokoro.".into(),
        reference_audio: None,
        language: None,
        voice: Some("af_heart".into()),
        extra: Default::default(),
    };
    engine
        .synthesize(&handle, &request, &wav)
        .map_err(|e| anyhow::anyhow!("synthesize failed: {e:#}"))?;
    let size = std::fs::metadata(&wav)?.len();
    println!("OK: wrote {} ({} bytes)", wav.display(), size);

    engine.unload(&handle)?;
    println!("Unloaded engine, VRAM should be free");

    Ok(())
}
