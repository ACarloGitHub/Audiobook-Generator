use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod chunker;
mod epub;
mod kokoro;
mod merger;
mod recovery;

#[derive(Parser, Debug)]
#[command(
    name = "kokoro-cli",
    version,
    about = "Standalone EPUB -> MP3 pipeline using Kokoro ONNX. Temporary prototype."
)]
struct Cli {
    /// Path to the input EPUB file. Ignored if --demo is set.
    #[arg(long)]
    epub: Option<PathBuf>,

    /// Directory containing the Kokoro ONNX model file
    /// (model_quantized.onnx, model_q8f16.onnx, or model.onnx).
    /// See README for the download instructions.
    #[arg(long, default_value = "./prototype/kokoro-cli/models")]
    model_dir: PathBuf,

    /// Directory containing the Kokoro voice packs (*.bin).
    #[arg(long, default_value = "./prototype/kokoro-cli/voices")]
    voices_dir: PathBuf,

    /// Voice id (e.g. af_heart, am_michael, bf_emma).
    /// See https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX for the full list.
    #[arg(long, default_value = "af_heart")]
    voice: String,

    /// Output directory. Created if missing.
    #[arg(long, default_value = "./out")]
    output: PathBuf,

    /// Target words per chunk. The chunker will not exceed this; it may be
    /// slightly under when a sentence does not fit cleanly.
    #[arg(long, default_value_t = 200)]
    max_words: usize,

    /// Skip the confirmation prompt before processing.
    #[arg(long, short = 'y')]
    yes: bool,

    /// Run a built-in demo sentence through the same pipeline. Skips EPUB.
    #[arg(long)]
    demo: bool,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("fatal: {e:#}");
            for cause in e.chain().skip(1) {
                tracing::error!("  caused by: {cause}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    std::fs::create_dir_all(&cli.output)
        .with_context(|| format!("failed to create output directory {}", cli.output.display()))?;

    let engine = kokoro::KokoroEngine::load(&cli.model_dir, &cli.voices_dir, &cli.voice)
        .with_context(|| "failed to load Kokoro ONNX")?;

    if cli.demo {
        return run_demo(&engine, &cli.output);
    }

    let epub_path = cli
        .epub
        .as_ref()
        .context("--epub is required unless --demo is set")?;

    let chapters = epub::extract_chapters(epub_path)
        .with_context(|| format!("failed to read EPUB {}", epub_path.display()))?;

    tracing::info!(
        "found {} chapter(s) in {}",
        chapters.len(),
        epub_path.display()
    );
    for (i, ch) in chapters.iter().enumerate() {
        tracing::info!("  [{}] {} ({} chars)", i + 1, ch.title, ch.text.len());
    }

    if !cli.yes {
        let proceed = dialoguer_confirm("Proceed?");
        if !proceed {
            tracing::info!("aborted by user");
            return Ok(());
        }
    }

    let mut recovery_state = recovery::RecoveryState::load(&cli.output);

    for (i, chapter) in chapters.iter().enumerate() {
        let chapter_dir = cli.output.join(sanitize_filename(&chapter.title));
        std::fs::create_dir_all(&chapter_dir)?;

        let chunks = chunker::chunk_text(&chapter.text, cli.max_words);
        tracing::info!(
            "chapter {}/{}: '{}' -> {} chunk(s)",
            i + 1,
            chapters.len(),
            chapter.title,
            chunks.len()
        );

        let mut wavs: Vec<PathBuf> = Vec::with_capacity(chunks.len());
        for (j, chunk) in chunks.iter().enumerate() {
            let wav_path = chapter_dir.join(format!("chunk_{:04}.wav", j + 1));

            if recovery_state.is_done(&chapter.title, j) && wav_path.exists() {
                tracing::debug!("chunk {}/{} already done, skipping", j + 1, chunks.len());
                wavs.push(wav_path);
                continue;
            }

            match engine.synthesize(chunk, &wav_path) {
                Ok(()) => {
                    recovery_state.mark_done(&chapter.title, j);
                    wavs.push(wav_path);
                }
                Err(e) => {
                    tracing::error!("chunk {}/{} failed: {e:#}", j + 1, chunks.len());
                    recovery_state.mark_failed(&chapter.title, j, chunk, &format!("{e:#}"));
                }
            }
        }

        if !wavs.is_empty() {
            let mp3_path = cli
                .output
                .join(format!("{}.mp3", sanitize_filename(&chapter.title)));
            merger::merge_wavs_to_mp3(&wavs, &mp3_path, &ffmpeg_exe()?)?;
            tracing::info!("merged {} chunk(s) -> {}", wavs.len(), mp3_path.display());
        }
    }

    Ok(())
}

fn run_demo(engine: &kokoro::KokoroEngine, output: &std::path::Path) -> Result<()> {
    let text = "Hello, this is a test of the Audiobook Generator prototype. \
                If you can hear this clearly, the Kokoro ONNX engine is working \
                end to end through the Rust pipeline.";
    let wav_path = output.join("demo.wav");
    engine.synthesize(text, &wav_path)?;
    let mp3_path = output.join("demo.mp3");
    let ffmpeg = ffmpeg_exe()?;
    merger::merge_wavs_to_mp3(&[wav_path.clone()], &mp3_path, &ffmpeg)?;
    tracing::info!("demo -> {}", mp3_path.display());
    Ok(())
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .replace(' ', "_")
        .to_lowercase()
}

fn ffmpeg_exe() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("FFMPEG") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
    }
    let local = std::env::current_dir()?
        .join("ffmpeg")
        .join("bin")
        .join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
    if local.exists() {
        return Ok(local);
    }
    which("ffmpeg")
}

fn which(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").context("PATH not set")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        });
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("ffmpeg not found on PATH and no bundled copy at ./ffmpeg/bin/")
}

fn dialoguer_confirm(prompt: &str) -> bool {
    use std::io::Write;
    eprint!("{prompt} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
