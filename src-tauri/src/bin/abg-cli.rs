//! abg-cli — command-line and MCP interface to Audiobook Generator.
//!
//! Two ways in (same tools underneath):
//!
//! 1. Direct commands:
//!      abg-cli status
//!      abg-cli synthesize --engine <id> [--text T | --text-file F]
//!          --out OUT.wav [--voice V] [--language L] [--ref REF.wav]
//!          [--max-chars N] [--param key=value]...
//!      abg-cli call <tool> '<json-args>'
//! 2. MCP server over stdio (newline-delimited JSON-RPC), for LM Studio
//!    and other MCP clients:
//!      abg-cli --mcp
//!
//! Tools: get_status, configure, synthesize, book, generate, recover.
//! The heavy lifting (engines, chunking, merging, GPU guard, recovery)
//! is shared with the desktop app through the library crate.

use anyhow::{bail, Context, Result};
use audiobook_generator_lib::base_plugin::{BaseTTSPlugin, SynthesizeRequest};
use audiobook_generator_lib::plugin_manager::{
    defaults_for, PluginManager, QwenPaths, VoxCpm2Paths,
};
use audiobook_generator_lib::{chunker, config, gpu_guard, input, merger, recovery, utils};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info};

fn init_paths() {
    // Same base dir Tauri uses for the desktop app (dirs::data_dir ==
    // %APPDATA% on Windows), so the CLI sees the same models and the
    // same storage override.
    let app_data = dirs::data_dir()
        .map(|d| d.join("com.patata.audiobookgenerator"))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&app_data);
    config::paths::set_app_data_dir(app_data);
    config::paths::load_storage_override();
}

/// Initialise the tracing subscriber so that all `info!`, `warn!`,
/// `debug!` calls in the library crate are actually captured.
/// Writes to `<app_data>/abg-cli.log` (append mode).
/// Returns the WorkerGuard which must be kept alive for the process lifetime.
fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_path = config::paths::app_data_dir().join("abg-cli.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap_or_else(|e| {
            eprintln!(
                "[abg-cli] FATAL: cannot open log file {}: {e}",
                log_path.display()
            );
            std::process::exit(1);
        });
    let (non_blocking, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_writer(non_blocking)
        .init();
    info!("=== abg-cli starting (log: {}) ===", log_path.display());
    guard
}

fn create_plugin(engine_id: &str) -> Option<Arc<dyn audiobook_generator_lib::base_plugin::BaseTTSPlugin>> {
    use audiobook_generator_lib::plugins;
    let qwen = plugins::qwen3tts::QwenPlugin::new(
        QwenPaths::from_app_data(&config::paths::storage_dir()),
        engine_id,
    );
    if qwen.is_installed() {
        return Some(Arc::new(qwen));
    }
    if engine_id.starts_with("VoxCPM2") {
        let vox = plugins::voxcpm2::VoxCpm2Plugin::new(
            VoxCpm2Paths::from_app_data(&config::paths::storage_dir()),
            engine_id,
        );
        if vox.is_installed() {
            return Some(Arc::new(vox));
        }
    }
    if engine_id.starts_with("OuteTTS") {
        let oute_dir = config::paths::models_dir().join("outetts");
        let oute = plugins::outetts::OuteTTSPlugin::new(oute_dir, engine_id);
        if oute.is_installed() {
            return Some(Arc::new(oute));
        }
    }
    None
}

fn status_json(pm: &PluginManager) -> serde_json::Value {
    let engines: Vec<serde_json::Value> = pm
        .catalogue()
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "display_name": e.display_name,
                "installed": e.installed,
                "license": e.license,
                "size_mb": e.size_mb,
            })
        })
        .collect();
    let gpus = gpu_guard::gpu_devices().unwrap_or_default();
    serde_json::json!({
        "storage_dir": config::paths::storage_dir().to_string_lossy(),
        "models_dir": config::paths::models_dir().to_string_lossy(),
        "gpu_devices": gpus,
        "engines": engines,
    })
}

// ---------------------------------------------------------------------
// Session (shared by CLI and MCP, persisted on disk)
// ---------------------------------------------------------------------

/// Persistent CLI/MCP session: engine and synthesis options chosen once
/// (via the `configure` tool) and reused by `synthesize` and `generate`,
/// so an external agent does not have to repeat them on every call.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Session {
    engine: Option<String>,
    voice: Option<String>,
    language: Option<String>,
    reference_audio: Option<String>,
    reference_transcript: Option<String>,
    #[serde(default)]
    params: HashMap<String, String>,
    book_path: Option<String>,
    book_title: Option<String>,
    output_dir: Option<String>,
}

fn session_path() -> PathBuf {
    config::paths::app_data_dir().join("cli_session.json")
}

fn session_load() -> Session {
    std::fs::read_to_string(session_path())
        .ok()
        .and_then(|b| serde_json::from_str(&b).ok())
        .unwrap_or_default()
}

fn session_save(s: &Session) -> Result<()> {
    std::fs::write(session_path(), serde_json::to_string_pretty(s)?)?;
    Ok(())
}

/// Inject the reference transcript into the engine-specific extra key:
/// qwen reads `ref_text`, voxcpm2 reads `prompt_text`. Never overwrites a
/// key the caller set explicitly.
fn inject_transcript(engine: &str, transcript: Option<&str>, extra: &mut HashMap<String, String>) {
    let Some(t) = transcript.filter(|t| !t.trim().is_empty()) else {
        return;
    };
    if engine.starts_with("Qwen3-TTS") && !extra.contains_key("ref_text") {
        extra.insert("ref_text".to_string(), t.to_string());
    }
    if engine.starts_with("VoxCPM2") && !extra.contains_key("prompt_text") {
        extra.insert("prompt_text".to_string(), t.to_string());
    }
}

/// Merge a JSON object of parameters into a string map (values may be
/// strings, numbers or booleans in the JSON).
fn merge_params(extra: &mut HashMap<String, String>, obj: &serde_json::Map<String, serde_json::Value>) {
    for (k, v) in obj {
        let val = v
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| v.to_string());
        extra.insert(k.clone(), val);
    }
}

// ---------------------------------------------------------------------
// Background job state (long-running generate / recover)
// ---------------------------------------------------------------------

/// State of a background job, persisted to `<app_data>/jobs/<job_id>.json`
/// so an MCP client (LM Studio) can poll progress without blocking the
/// server's request loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobState {
    job_id: String,
    tool: String,
    status: String,
    started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    progress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    book_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gen_params: Option<GenParams>,
}

/// Generation parameters stored in the job file so the detached
/// `internal-generate` process can read them.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GenParams {
    engine: String,
    epub: String,
    output_dir: String,
    max_words: usize,
    max_chars: usize,
    voice: Option<String>,
    language: Option<String>,
    reference: Option<String>,
    only: Option<Vec<String>>,
    extra: HashMap<String, String>,
    delete_chunks: bool,
}

fn jobs_dir() -> PathBuf {
    config::paths::app_data_dir().join("jobs")
}

fn generate_job_id(prefix: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{prefix}_{secs}")
}

fn job_file_path(job_id: &str) -> PathBuf {
    jobs_dir().join(format!("{job_id}.json"))
}

fn save_job_state(state: &JobState) {
    let _ = std::fs::create_dir_all(jobs_dir());
    if let Ok(body) = serde_json::to_string(state) {
        let _ = std::fs::write(job_file_path(&state.job_id), body);
    }
}

fn load_job_state(job_id: &str) -> Option<JobState> {
    let body = std::fs::read_to_string(job_file_path(job_id)).ok()?;
    serde_json::from_str(&body).ok()
}

fn update_job_progress(job_id: &str, msg: &str) {
    if let Some(mut state) = load_job_state(job_id) {
        state.progress = Some(msg.to_string());
        save_job_state(&state);
    }
}

fn finish_job(job_id: &str, outcome: Result<String, String>) {
    if let Some(mut state) = load_job_state(job_id) {
        state.finished_at = Some(recovery::now_stamp());
        match outcome {
            Ok(msg) => {
                state.status = "done".to_string();
                state.result = Some(msg);
            }
            Err(err) => {
                state.status = "failed".to_string();
                state.error = Some(err);
            }
        }
        save_job_state(&state);
    }
}

// ---------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------

fn tool_configure(pm: &PluginManager, args: &serde_json::Value) -> Result<String> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("set");
    match action {
        "list_engines" => {
            let engines: Vec<serde_json::Value> = pm
                .catalogue()
                .into_iter()
                .filter(|e| e.installed)
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "name": e.display_name,
                        "voice_cloning": e.voice_cloning,
                        "languages": e.languages,
                        "preset_voices": e.voices.len(),
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&engines)?)
        }
        "list_voices" => {
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .context("missing 'engine'")?;
            let d = defaults_for(engine);
            if d.voices.is_empty() {
                return Ok(format!(
                    "Engine '{engine}' has no preset voices (voice cloning / design mode)."
                ));
            }
            let voices: Vec<serde_json::Value> = d
                .voices
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "id": v.id,
                        "name": v.display_name,
                        "native_language": v.language,
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&voices)?)
        }
        "get_parameters" => {
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .context("missing 'engine'")?;
            let d = defaults_for(engine);
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "engine": d.engine_id,
                "chunk_max_chars": d.chunk_max_chars,
                "supported_languages": d.supported_languages,
                "voice_cloning": d.voice_cloning,
                "needs_reference_transcript": d.needs_reference_transcript,
                "generation_parameters": d.generation,
            }))?)
        }
        "set" => {
            let mut s = session_load();
            let mut notes: Vec<String> = Vec::new();
            if let Some(e) = args.get("engine").and_then(|v| v.as_str()) {
                let installed = pm.catalogue().iter().any(|c| c.id == e && c.installed);
                if !installed {
                    bail!("engine '{e}' is not installed; run configure list_engines");
                }
                if s.engine.as_deref() != Some(e) {
                    s.voice = None;
                    notes.push("voice reset (engine changed)".to_string());
                }
                s.engine = Some(e.to_string());
            }
            if let Some(v) = args.get("voice").and_then(|v| v.as_str()) {
                s.voice = Some(v.to_string());
            }
            if let Some(v) = args.get("language").and_then(|v| v.as_str()) {
                s.language = Some(v.to_string());
            }
            if let Some(v) = args.get("reference_audio").and_then(|v| v.as_str()) {
                s.reference_audio = Some(v.to_string());
            }
            if let Some(v) = args.get("reference_transcript").and_then(|v| v.as_str()) {
                s.reference_transcript = Some(v.to_string());
            }
            if let Some(v) = args.get("output_dir").and_then(|v| v.as_str()) {
                s.output_dir = Some(v.to_string());
            }
            if let Some(obj) = args.get("params").and_then(|v| v.as_object()) {
                merge_params(&mut s.params, obj);
            }
            if let Some(engine) = s.engine.clone() {
                let d = defaults_for(&engine);
                if d.needs_reference_transcript
                    && s.reference_audio.is_some()
                    && s
                        .reference_transcript
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .is_empty()
                {
                    notes.push(format!(
                        "WARNING: engine '{engine}' needs the transcript of the reference audio for good quality; ask the user for it and set reference_transcript."
                    ));
                }
            }
            session_save(&s)?;
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "session": s,
                "notes": notes,
            }))?)
        }
        other => bail!("unknown configure action '{other}'"),
    }
}

struct SynthArgs {
    engine: String,
    text: Option<String>,
    text_file: Option<PathBuf>,
    out: PathBuf,
    voice: Option<String>,
    language: Option<String>,
    reference: Option<String>,
    max_chars: Option<usize>,
    extra: HashMap<String, String>,
}

async fn run_synthesize(args: SynthArgs) -> Result<PathBuf> {
    let text = match (args.text, args.text_file) {
        (Some(t), _) => t,
        (None, Some(f)) => std::fs::read_to_string(&f)
            .with_context(|| format!("reading text file {}", f.display()))?,
        (None, None) => bail!("either --text or --text-file is required"),
    };
    if text.trim().is_empty() {
        bail!("empty input text");
    }

    // GPU-only rule: same guard as the desktop app.
    gpu_guard::ensure_gpu()?;

    let plugin = create_plugin(&args.engine)
        .ok_or_else(|| anyhow::anyhow!("engine '{}' is not installed or model files missing", args.engine))?;
    let handle = plugin.load_model(&args.engine).await?;

    let defaults = defaults_for(&args.engine);
    let max_chars = args.max_chars.unwrap_or(defaults.chunk_max_chars as usize);
    let chunks = chunker::chunk_text(&text, 1000, max_chars);
    if chunks.is_empty() {
        bail!("empty text after chunking");
    }

    let temp_dir = args
        .out
        .parent()
        .map(|p| p.join("_cli_chunks"))
        .unwrap_or_else(|| PathBuf::from("_cli_chunks"));
    std::fs::create_dir_all(&temp_dir)?;

    let mut wavs: Vec<PathBuf> = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_path = temp_dir.join(format!("chunk_{:04}.wav", i + 1));
        let request = SynthesizeRequest {
            text: chunk.clone(),
            output_path: chunk_path.to_string_lossy().to_string(),
            reference_audio: args.reference.clone(),
            language: args.language.clone(),
            voice: args.voice.clone(),
            extra: args.extra.clone(),
        };
        plugin
            .synthesize(&handle, &request)
            .await
            .with_context(|| format!("synthesis failed on chunk {}", i + 1))?;
        wavs.push(chunk_path);
    }
    let _ = plugin.unload(&handle).await;

    if wavs.len() == 1 {
        std::fs::rename(&wavs[0], &args.out)?;
    } else {
        let ffmpeg = merger::find_ffmpeg()?;
        merger::merge_wavs_to_wav(&wavs, &args.out, &ffmpeg)?;
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(args.out.canonicalize().unwrap_or(args.out))
}

async fn tool_synthesize(args: &serde_json::Value) -> Result<String> {
    let s = session_load();
    let engine = args
        .get("engine")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or(s.engine.clone())
        .context("missing 'engine' (none configured in the session)")?;
    let out = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .context("missing 'output_path'")?;
    let mut extra = s.params.clone();
    if let Some(obj) = args.get("extra").and_then(|v| v.as_object()) {
        merge_params(&mut extra, obj);
    }
    inject_transcript(&engine, s.reference_transcript.as_deref(), &mut extra);
    let pick = |key: &str, fallback: &Option<String>| {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| fallback.clone())
    };
    let path = run_synthesize(
        SynthArgs {
            engine,
            text: args.get("text").and_then(|v| v.as_str()).map(String::from),
            text_file: args
                .get("text_file")
                .and_then(|v| v.as_str())
                .map(PathBuf::from),
            out: PathBuf::from(out),
            voice: pick("voice", &s.voice),
            language: pick("language", &s.language),
            reference: pick("reference_audio", &s.reference_audio),
            max_chars: args
                .get("max_chars")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize),
            extra,
        },
    )
    .await?;
    Ok(format!("WAV written to {}", path.display()))
}

fn tool_book(args: &serde_json::Value) -> Result<String> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("chapters");
    match action {
        "load" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .context("missing 'path'")?;
            let p = PathBuf::from(path);
            let chapters = input::extract_chapters_from(&p)?;
            // Title: explicit argument, otherwise the file name (same
            // fallback the desktop app uses when metadata is missing).
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "Untitled".to_string());
            let mut s = session_load();
            s.book_path = Some(path.to_string());
            s.book_title = Some(title.clone());
            session_save(&s)?;
            Ok(format_book_listing(&title, &chapters))
        }
        "chapters" => {
            let s = session_load();
            let path = s
                .book_path
                .as_deref()
                .context("no book loaded; use book action=load first")?;
            let chapters = input::extract_chapters_from(Path::new(path))?;
            Ok(format_book_listing(
                s.book_title.as_deref().unwrap_or("book"),
                &chapters,
            ))
        }
        other => bail!("unknown book action '{other}'"),
    }
}

fn format_book_listing(title: &str, chapters: &[input::Chapter]) -> String {
    let names: Vec<String> = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c.title))
        .collect();
    format!(
        "'{title}' ({} chapters):\n{}",
        chapters.len(),
        names.join("\n")
    )
}

/// Same rule as the desktop app: intermediate chunk folders are deleted
/// only when there are no failed chunks; otherwise everything is kept
/// and the failed chapters are reported (they feed the recover tool).
fn cleanup_cli(out: &Path) -> String {
    let book = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| out.display().to_string());
    let has_failures = recovery::RecoveryState::load(out)
        .map(|s| !s.failed.is_empty())
        .unwrap_or(false);
    if has_failures {
        let failed = recovery::RecoveryState::load(out)
            .map(|s| {
                let mut v: Vec<String> = s.failed.keys().cloned().collect();
                v.sort();
                v.join(", ")
            })
            .unwrap_or_else(|_| "unknown".to_string());
        return format!(
            "\nCleanup skipped: failed chunks present in '{book}' (chapters: {failed}). Intermediate chunks preserved for recovery."
        );
    }
    if let Ok(entries) = std::fs::read_dir(out) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
    }
    format!("\nIntermediate chunks deleted for '{book}' (no failed chunks).")
}

async fn tool_generate(args: &serde_json::Value) -> Result<String> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("start");
    match action {
        "status" => {
            let job_id = args
                .get("job_id")
                .and_then(|v| v.as_str())
                .context("missing 'job_id'")?;
            let state = load_job_state(job_id)
                .with_context(|| format!("job '{job_id}' not found"))?;
            Ok(serde_json::to_string_pretty(&state)?)
        }
        "stop" => {
            let job_id = args
                .get("job_id")
                .and_then(|v| v.as_str())
                .context("missing 'job_id'")?;
            let state = load_job_state(job_id)
                .with_context(|| format!("job '{job_id}' not found"))?;
            if state.status != "running" {
                return Ok(format!("Job '{job_id}' is not running (status: {})", state.status));
            }
            if let Some(pid) = state.pid {
                #[cfg(windows)]
                {
                    let kill = std::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/T", "/F"])
                        .output();
                    match kill {
                        Ok(o) if o.status.success() => {
                            if let Some(mut s) = load_job_state(job_id) {
                                s.status = "stopped".to_string();
                                s.finished_at = Some(recovery::now_stamp());
                                s.progress = Some("Stopped by user".to_string());
                                save_job_state(&s);
                            }
                            Ok(format!("Job '{job_id}' (pid {pid}) killed."))
                        }
                        Ok(o) => Ok(format!(
                            "Kill command failed: {}",
                            String::from_utf8_lossy(&o.stderr)
                        )),
                        Err(e) => Ok(format!("Failed to run taskkill: {e}")),
                    }
                }
                #[cfg(not(windows))]
                {
                    let kill = std::process::Command::new("kill")
                        .args(["-TERM", &pid.to_string()])
                        .output();
                    match kill {
                        Ok(o) if o.status.success() => {
                            if let Some(mut s) = load_job_state(job_id) {
                                s.status = "stopped".to_string();
                                s.finished_at = Some(recovery::now_stamp());
                                s.progress = Some("Stopped by user".to_string());
                                save_job_state(&s);
                            }
                            Ok(format!("Job '{job_id}' (pid {pid}) killed."))
                        }
                        _ => Ok(format!("Failed to kill pid {pid}")),
                    }
                }
            } else {
                Ok(format!("Job '{job_id}' has no PID; cannot stop."))
            }
        }
        "start" => {
            let s = session_load();
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(s.engine.clone())
                .context("missing engine; configure one first (configure action=set)")?;
            let book_path = args
                .get("book_path")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(s.book_path.clone())
                .context("no book loaded; use the book tool first")?;
            let title =
                s.book_title.clone().unwrap_or_else(|| "audiobook".to_string());
            let output_dir = args
                .get("output_dir")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .or_else(|| s.output_dir.as_ref().map(PathBuf::from))
                .unwrap_or_else(|| {
                    PathBuf::from(config::paths::output_base_dir())
                        .join(utils::sanitize_filename(&title))
                });
            let only: Option<Vec<String>> = args
                .get("chapters")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                });
            let delete_chunks = args
                .get("delete_intermediate_chunks")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let defaults = defaults_for(&engine);
            let max_chars = args
                .get("max_chars")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(defaults.chunk_max_chars as usize);
            let max_words = args
                .get("max_words")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(1000);
            let mut extra = s.params.clone();
            if let Some(obj) = args.get("extra").and_then(|v| v.as_object()) {
                merge_params(&mut extra, obj);
            }
            inject_transcript(&engine, s.reference_transcript.as_deref(), &mut extra);
            let voice = args
                .get("voice")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(s.voice.clone());
            let language = args
                .get("language")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(s.language.clone());
            let reference = args
                .get("reference_audio")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(s.reference_audio.clone());

            // Validate before spawning the background task.
            gpu_guard::ensure_gpu()?;
            create_plugin(&engine)
                .ok_or_else(|| anyhow::anyhow!("engine '{engine}' is not installed or model files missing"))?;
            std::fs::create_dir_all(&output_dir)?;

            let job_id = generate_job_id("gen");
            let gen_params = GenParams {
                engine: engine.clone(),
                epub: book_path.clone(),
                output_dir: output_dir.to_string_lossy().to_string(),
                max_words,
                max_chars,
                voice,
                language,
                reference,
                only,
                extra,
                delete_chunks,
            };
            let job = JobState {
                job_id: job_id.clone(),
                tool: "generate".to_string(),
                status: "running".to_string(),
                started_at: recovery::now_stamp(),
                finished_at: None,
                progress: Some("Starting…".to_string()),
                result: None,
                error: None,
                book_dir: Some(output_dir.to_string_lossy().to_string()),
                engine: Some(engine.clone()),
                pid: None,
                gen_params: Some(gen_params),
            };
            save_job_state(&job);

            // Spawn a DETACHED child process (`abg-cli internal-generate`)
            // that runs the full generation independently. This process
            // survives even if the MCP server (abg-cli --mcp) exits,
            // which is essential because some MCP clients restart the
            // server between requests.
            //
            // CREATE_BREAKAWAY_FROM_JOB is critical: without it, the child
            // inherits the parent's Job Object. When LM Studio restarts
            // the MCP server, Windows kills all processes in the Job
            // Object — including this child — mid-generation.
            let exe = std::env::current_exe()
                .context("cannot find current executable")?;
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("internal-generate")
               .arg("--job-id").arg(&job_id)
               .stdin(std::process::Stdio::null())
               .stdout(std::process::Stdio::null())
               .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
                const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(
                    CREATE_BREAKAWAY_FROM_JOB
                        | CREATE_NEW_PROCESS_GROUP
                        | CREATE_NO_WINDOW,
                );
            }
            #[cfg(not(windows))]
            {
                crate::utils::hide_console_window(&mut cmd);
            }
            let child = cmd.spawn()
                .context("failed to spawn internal-generate process")?;
            let pid = child.id();
            // Detach: do NOT assign_child (we want this process to
            // outlive the MCP server).
            drop(child);

            // Record the PID so `stop` can kill it later.
            if let Some(mut state) = load_job_state(&job_id) {
                state.pid = Some(pid);
                save_job_state(&state);
            }

            let output_dir_resp = output_dir.clone();
            Ok(format!(
                "Generation started (job_id: {job_id}, pid: {pid}). Output: {}. Poll with: generate action=status, job_id=\"{job_id}\". Stop with: generate action=stop, job_id=\"{job_id}\".",
                output_dir_resp.display()
            ))
        }
        other => bail!("unknown generate action '{other}'"),
    }
}

/// Run chunk retry in the background, updating job state as it goes.
async fn run_retry(book_dir: PathBuf, chapter_filter: Option<String>, job_id: String) {
    let result: Result<String> = async {
        let mut state = recovery::RecoveryState::load(&book_dir)
            .with_context(|| format!("loading recovery state from {}", book_dir.display()))?;
        if state.failed.is_empty() {
            bail!("nothing to recover in {}", book_dir.display());
        }
        let s = session_load();
        let engine = state
            .meta
            .engine_id
            .clone()
            .or(s.engine.clone())
            .context("no engine recorded in the recovery file and none configured")?;
        let mut extra = if state.meta.extra.is_empty() {
            s.params.clone()
        } else {
            state.meta.extra.clone()
        };
        inject_transcript(&engine, s.reference_transcript.as_deref(), &mut extra);
        let reference = state
            .meta
            .reference_audio
            .clone()
            .or(s.reference_audio.clone())
            .filter(|x| !x.is_empty());
        let voice = state.meta.voice.clone().or(s.voice.clone());
        let language = state.meta.language.clone().or(s.language.clone());

        gpu_guard::ensure_gpu()?;
        let plugin = create_plugin(&engine)
            .ok_or_else(|| anyhow::anyhow!("engine '{engine}' is not installed or model files missing"))?;
        let handle = plugin.load_model(&engine).await?;

        let chapters: Vec<String> = match &chapter_filter {
            Some(c) => vec![c.clone()],
            None => {
                let mut v: Vec<String> = state.failed.keys().cloned().collect();
                v.sort();
                v
            }
        };
        let mut ok = 0usize;
        let mut failed_again = 0usize;
        for chapter in chapters {
            let Some(fails) = state.failed.get(&chapter).cloned() else {
                continue;
            };
            let chapter_dir = book_dir.join(utils::sanitize_filename(&chapter));
            std::fs::create_dir_all(&chapter_dir)?;
            let total = fails.len();
            for (i, f) in fails.into_iter().enumerate() {
                let wav = chapter_dir.join(format!("chunk_{:04}.wav", f.chunk_index + 1));
                let request = SynthesizeRequest {
                    text: f.text.clone(),
                    output_path: wav.to_string_lossy().to_string(),
                    reference_audio: reference.clone(),
                    language: language.clone(),
                    voice: voice.clone(),
                    extra: extra.clone(),
                };
                match plugin.synthesize(&handle, &request).await {
                    Ok(()) => {
                        state.remove_failed(&chapter, f.chunk_index);
                        if !state.is_done(&chapter, f.chunk_index) {
                            state.mark_done(&chapter, f.chunk_index);
                        }
                        ok += 1;
                    }
                    Err(e) => {
                        state.update_failed(&chapter, f.chunk_index, &f.text, &format!("{e:#}"));
                        failed_again += 1;
                    }
                }
                state.save(&book_dir)?;
                update_job_progress(
                    &job_id,
                    &format!("Chapter '{chapter}': retrying chunk {} of {total}", i + 1),
                );
            }
        }
        let _ = plugin.unload(&handle).await;
        Ok(format!(
            "retry done: {ok} chunk(s) recovered, {failed_again} still failing. When a chapter has no failures left, merge it with recover action=merge."
        ))
    }
    .await;
    finish_job(&job_id, result.map_err(|e| format!("{e:#}")));
}

async fn tool_recover(args: &serde_json::Value) -> Result<String> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
    match action {
        "list" => {
            let root = args
                .get("root_dir")
                .and_then(|v| v.as_str())
                .context("missing 'root_dir'")?;
            let mut report = String::new();
            let mut found = 0usize;
            for entry in std::fs::read_dir(root)
                .with_context(|| format!("reading {root}"))?
                .flatten()
            {
                let path = entry.path();
                if !path.is_dir() || !path.join("failed_chunks.json").exists() {
                    continue;
                }
                let Ok(state) = recovery::RecoveryState::load(&path) else {
                    continue;
                };
                if state.failed.is_empty() {
                    continue;
                }
                found += 1;
                report.push_str(&format!("book_dir: {}\n", path.display()));
                let mut chapters: Vec<&String> = state.failed.keys().collect();
                chapters.sort();
                for ch in chapters {
                    let fails = &state.failed[ch];
                    let idx: Vec<usize> = fails.iter().map(|f| f.chunk_index + 1).collect();
                    report.push_str(&format!(
                        "  chapter '{ch}': {} failed chunk(s), indices {idx:?}\n",
                        fails.len()
                    ));
                }
            }
            if found == 0 {
                Ok("no interrupted generations found".to_string())
            } else {
                Ok(report)
            }
        }
        "retry" => {
            let book_dir = PathBuf::from(
                args.get("book_dir")
                    .and_then(|v| v.as_str())
                    .context("missing 'book_dir'")?,
            );
            let chapter_filter = args
                .get("chapter")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Validate before spawning.
            let state = recovery::RecoveryState::load(&book_dir)?;
            if state.failed.is_empty() {
                bail!("nothing to recover in {}", book_dir.display());
            }

            let job_id = generate_job_id("rec");
            let job = JobState {
                job_id: job_id.clone(),
                tool: "recover".to_string(),
                status: "running".to_string(),
                started_at: recovery::now_stamp(),
                finished_at: None,
                progress: Some("Loading model…".to_string()),
                result: None,
                error: None,
                book_dir: Some(book_dir.to_string_lossy().to_string()),
                engine: state.meta.engine_id.clone(),
                pid: None,
                gen_params: None,
            };
            save_job_state(&job);

            tokio::spawn(run_retry(book_dir, chapter_filter, job_id.clone()));

            Ok(format!(
                "Retry started (job_id: {job_id}). Poll with: recover action=status, job_id=\"{job_id}\"."
            ))
        }
        "merge" => {
            let book_dir = PathBuf::from(
                args.get("book_dir")
                    .and_then(|v| v.as_str())
                    .context("missing 'book_dir'")?,
            );
            let chapter = args
                .get("chapter")
                .and_then(|v| v.as_str())
                .context("missing 'chapter'")?;
            let chapter_dir = book_dir.join(utils::sanitize_filename(chapter));
            let wavs = merger::collect_chapter_wavs(&chapter_dir);
            if wavs.is_empty() {
                bail!("no chunk WAVs found in {}", chapter_dir.display());
            }
            let ffmpeg = merger::find_ffmpeg()?;
            let mp3 = book_dir.join(format!("{}.mp3", utils::sanitize_filename(chapter)));
            merger::merge_wavs_to_mp3(&wavs, &mp3, &ffmpeg)?;
            let mut state = recovery::RecoveryState::load(&book_dir)?;
            state.clear_chapter(chapter);
            if state.failed.is_empty() && state.done.is_empty() {
                recovery::RecoveryState::remove_file_if_empty(&book_dir, &state)?;
            } else {
                state.save(&book_dir)?;
            }
            Ok(format!("merged {} chunk(s) into {}", wavs.len(), mp3.display()))
        }
        "status" => {
            let job_id = args
                .get("job_id")
                .and_then(|v| v.as_str())
                .context("missing 'job_id'")?;
            let state = load_job_state(job_id)
                .with_context(|| format!("job '{job_id}' not found"))?;
            Ok(serde_json::to_string_pretty(&state)?)
        }
        other => bail!("unknown recover action '{other}'"),
    }
}

async fn dispatch_tool(pm: &PluginManager, name: &str, args: &serde_json::Value) -> Result<String> {
    match name {
        "get_status" => Ok(serde_json::to_string_pretty(&status_json(pm))?),
        "configure" => tool_configure(pm, args),
        "synthesize" => tool_synthesize(args).await,
        "book" => tool_book(args),
        "generate" => tool_generate(args).await,
        "recover" => tool_recover(args).await,
        other => bail!("unknown tool '{other}'"),
    }
}

// ---------------------------------------------------------------------
// Direct CLI parsing
// ---------------------------------------------------------------------

fn print_usage() {
    eprintln!(
        "abg-cli — Audiobook Generator command line\n\
         \n\
         Usage:\n\
         \x20 abg-cli status\n\
         \x20 abg-cli synthesize --engine <id> [--text T | --text-file F] --out OUT.wav\n\
         \x20     [--voice V] [--language L] [--ref REF.wav] [--max-chars N] [--param k=v]...\n\
         \x20 abg-cli call <tool> '<json-args>'\n\
         \x20 abg-cli --mcp\n\
         \n\
         Tools: get_status, configure, synthesize, book, generate, recover.\n\
         Engine ids: run `abg-cli status` to list them."
    );
}

fn parse_kv(pairs: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for p in pairs {
        if let Some((k, v)) = p.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

// ---------------------------------------------------------------------
// MCP server (newline-delimited JSON-RPC over stdio)
// ---------------------------------------------------------------------

fn mcp_tools() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "get_status",
            "description": "Get Audiobook Generator status: storage folder, GPU devices, installed TTS engines and models.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "configure",
            "description": "Configure the synthesis session (engine, voice, language, reference audio/transcript, engine parameters). Settings persist across calls. Actions: list_engines, list_voices, get_parameters, set.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list_engines", "list_voices", "get_parameters", "set"] },
                    "engine": { "type": "string", "description": "Engine id (see list_engines)" },
                    "voice": { "type": "string", "description": "Preset voice id (see list_voices)" },
                    "language": { "type": "string" },
                    "reference_audio": { "type": "string", "description": "Path to a reference WAV for voice cloning" },
                    "reference_transcript": { "type": "string", "description": "Transcript of the reference audio (required by some engines for good quality)" },
                    "output_dir": { "type": "string", "description": "Default output folder for generate" },
                    "params": { "type": "object", "description": "Engine parameters as key/value, e.g. {\"temperature\": \"0.7\"}" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "synthesize",
            "description": "Synthesize speech to a WAV file. Uses the configured session as fallback for engine, voice, language, reference and parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "engine": { "type": "string", "description": "Engine id (optional if configured)" },
                    "text": { "type": "string", "description": "Text to synthesize" },
                    "text_file": { "type": "string", "description": "Path to a UTF-8 text file (alternative to text)" },
                    "output_path": { "type": "string", "description": "Destination WAV file path" },
                    "voice": { "type": "string" },
                    "language": { "type": "string" },
                    "reference_audio": { "type": "string", "description": "Path to a reference WAV for voice cloning" },
                    "max_chars": { "type": "integer" },
                    "extra": { "type": "object", "description": "Engine parameters, e.g. {\"temperature\": \"0.7\"}" }
                },
                "required": ["output_path"]
            }
        },
        {
            "name": "book",
            "description": "Load a document (epub, txt, md, docx, json) and inspect its chapters. Actions: load, chapters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["load", "chapters"] },
                    "path": { "type": "string", "description": "Document path (load)" },
                    "title": { "type": "string", "description": "Book title (load; defaults to the file name)" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "generate",
            "description": "Generate the audiobook with the configured session. Requires a loaded book and a configured engine. Action 'start' launches the generation in the background and returns immediately with a job_id; poll with action 'status' to track progress, completion, or errors.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["start", "status", "stop"], "description": "start launches generation as a detached background process and returns immediately with a job_id and pid; stop kills a running job by pid; status reads the job state file for progress, completion, or errors." },
                    "job_id": { "type": "string", "description": "Job identifier returned by start (required for status)" },
                    "output_dir": { "type": "string", "description": "Output folder (defaults to the configured one, or the app output folder)" },
                    "chapters": { "type": "array", "items": { "type": "string" }, "description": "Chapter titles to convert (omit for the whole book)" },
                    "max_chars": { "type": "integer", "description": "Chunk size in characters (defaults to the engine limit)" },
                    "delete_intermediate_chunks": { "type": "boolean", "description": "Delete chunk WAV folders after success (kept when failures exist)" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "recover",
            "description": "Repair interrupted generations. Actions: list (root_dir) shows books with failed chunks; retry (book_dir, optional chapter) re-synthesizes failed chunks in the background and returns a job_id; status (job_id) checks retry progress; merge (book_dir, chapter) rebuilds the chapter MP3.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "retry", "merge", "status"] },
                    "job_id": { "type": "string", "description": "Job identifier returned by retry (required for status)" },
                    "root_dir": { "type": "string", "description": "Folder to scan for interrupted books (list)" },
                    "book_dir": { "type": "string", "description": "Output folder of the book (retry, merge)" },
                    "chapter": { "type": "string", "description": "Chapter title (optional for retry, required for merge)" }
                },
                "required": ["action"]
            }
        }
    ])
}

async fn mcp_handle(
    pm: &PluginManager,
    req: &serde_json::Value,
) -> Option<serde_json::Value> {
    let method = req.get("method").and_then(|m| m.as_str())?;
    let id = req.get("id").cloned();
    let result: Result<serde_json::Value> = match method {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "audiobook-generator", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(serde_json::json!({})),
        "tools/list" => Ok(serde_json::json!({ "tools": mcp_tools() })),
        "tools/call" => {
            let params = req.get("params").cloned().unwrap_or_default();
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let args = params.get("arguments").cloned().unwrap_or_default();
            match dispatch_tool(pm, &name, &args).await {
                Ok(t) => Ok(serde_json::json!({
                    "content": [{ "type": "text", "text": t }],
                    "isError": false
                })),
                Err(e) => Ok(serde_json::json!({
                    "content": [{ "type": "text", "text": format!("{e:#}") }],
                    "isError": true
                })),
            }
        }
        // Notifications and unknown methods: no result.
        _ => return None,
    };

    // Requests without an id are notifications: never reply.
    let id = id?;
    let response = match result {
        Ok(r) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": format!("{e:#}") }
        }),
    };
    Some(response)
}

async fn run_mcp(pm: &PluginManager) -> Result<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[abg-cli mcp] invalid JSON: {e}");
                continue;
            }
        };
        if let Some(resp) = mcp_handle(pm, &req).await {
            let mut out = stdout.lock();
            writeln!(out, "{}", resp)?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Run generation as a detached process. Reads all parameters from the
/// job state file, runs the synthesis, and writes progress/result back.
/// This function exists so that the MCP server can respond immediately
/// while generation continues in a separate OS process that survives
/// even if the MCP server (abg-cli --mcp) exits.
async fn run_internal_generate(job_id: String) -> Result<String> {
    let state = load_job_state(&job_id)
        .with_context(|| format!("job '{job_id}' not found"))?;
    let params = state.gen_params
        .ok_or_else(|| anyhow::anyhow!("job '{job_id}' has no gen_params"))?;

    let ffmpeg = merger::find_ffmpeg()?;

    let job_id_cb = job_id.clone();
    let cb: Box<dyn FnMut(&str) + Send> =
        Box::new(move |msg: &str| update_job_progress(&job_id_cb, msg));

    let out_msg = PathBuf::from(&params.output_dir);

    let n: usize = if params.engine.starts_with("Qwen3-TTS") {
        let paths = QwenPaths::from_app_data(&config::paths::storage_dir());
        let engine_c = params.engine.clone();
        let epub = PathBuf::from(&params.epub);
        let out = PathBuf::from(&params.output_dir);
        tokio::task::spawn_blocking(move || {
            let q = audiobook_generator_lib::plugins::qwen3tts::QwenPlugin::new(paths, &engine_c);
            audiobook_generator_lib::plugins::qwen3tts::synthesize_book(
                &q, &epub, &out, params.max_words, params.max_chars, &ffmpeg,
                params.voice.as_deref(), params.language.as_deref(),
                params.reference.as_deref(), params.only.as_deref(),
                &params.extra, Some(cb),
            )
        })
        .await
        .context("synthesis task panicked")??
    } else if params.engine.starts_with("VoxCPM2") {
        let paths = VoxCpm2Paths::from_app_data(&config::paths::storage_dir());
        let engine_c = params.engine.clone();
        let epub = PathBuf::from(&params.epub);
        let out = PathBuf::from(&params.output_dir);
        tokio::task::spawn_blocking(move || {
            let p = audiobook_generator_lib::plugins::voxcpm2::VoxCpm2Plugin::new(paths, &engine_c);
            audiobook_generator_lib::plugins::voxcpm2::synthesize_book(
                &p, &epub, &out, params.max_words, params.max_chars, &ffmpeg,
                params.reference.as_deref(), params.only.as_deref(),
                &params.extra, Some(cb),
            )
        })
        .await
        .context("synthesis task panicked")??
    } else if params.engine.starts_with("OuteTTS") {
        let models_dir = config::paths::models_dir().join("outetts");
        let engine_c = params.engine.clone();
        let epub = PathBuf::from(&params.epub);
        let out = PathBuf::from(&params.output_dir);
        tokio::task::spawn_blocking(move || {
            let p = audiobook_generator_lib::plugins::outetts::OuteTTSPlugin::new(models_dir, &engine_c);
            audiobook_generator_lib::plugins::outetts::synthesize_book(
                &p, &epub, &out, params.max_words, params.max_chars, &ffmpeg,
                params.only.as_deref(), &params.extra, Some(cb),
            )
        })
        .await
        .context("synthesis task panicked")??
    } else {
        bail!("engine '{}' does not support book generation", params.engine);
    };

    let mut msg = format!("Done: {n} chunks synthesized into {}", out_msg.display());
    if let Ok(rec_state) = recovery::RecoveryState::load(&out_msg) {
        if !rec_state.failed.is_empty() {
            let mut ch: Vec<String> = rec_state.failed.keys().cloned().collect();
            ch.sort();
            msg.push_str(&format!(
                "\nWARNING: failed chunks in chapters: {}. Use recover action=retry with book_dir '{}' to fix them, then action=merge.",
                ch.join(", "),
                out_msg.display()
            ));
        }
    }
    if params.delete_chunks {
        msg.push_str(&cleanup_cli(&out_msg));
    }
    Ok(msg)
}

#[tokio::main]
async fn main() -> Result<()> {
    init_paths();
    let _log_guard = init_logging();
    audiobook_generator_lib::job_object::init();
    let pm = PluginManager::new(config::paths::app_data_dir());

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        print_usage();
        std::process::exit(2);
    };

    match cmd.as_str() {
        "--mcp" => run_mcp(&pm).await,
        "internal-generate" => {
            let job_id = args.get(1).context("missing --job-id")?;
            let job_id = if job_id == "--job-id" {
                args.get(2).context("missing job_id value")?.clone()
            } else {
                bail!("expected --job-id, got '{}'", job_id)
            };
            info!("[internal-generate] starting job {job_id}");
            let result = run_internal_generate(job_id.clone()).await;
            match &result {
                Ok(msg) => info!("[internal-generate] done: {msg}"),
                Err(e) => error!("[internal-generate] failed: {e:#}"),
            }
            finish_job(&job_id, result.map_err(|e| format!("{e:#}")));
            Ok(())
        }
        "status" => {
            println!("{}", serde_json::to_string_pretty(&status_json(&pm))?);
            Ok(())
        }
        "call" => {
            let tool = args.get(1).context("missing tool name")?;
            let json: serde_json::Value = match args.get(2) {
                Some(s) => serde_json::from_str(s).context("arguments must be a JSON object")?,
                None => serde_json::json!({}),
            };
            let out = dispatch_tool(&pm, tool, &json).await?;
            println!("{out}");
            Ok(())
        }
        "synthesize" => {
            let mut s = SynthArgs {
                engine: String::new(),
                text: None,
                text_file: None,
                out: PathBuf::new(),
                voice: None,
                language: None,
                reference: None,
                max_chars: None,
                extra: HashMap::new(),
            };
            let mut kv_pairs: Vec<String> = Vec::new();
            let mut i = 1;
            while i < args.len() {
                let flag = args[i].as_str();
                let take = |i: &mut usize| -> Result<String> {
                    *i += 1;
                    args.get(*i)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("missing value for {}", flag))
                };
                match flag {
                    "--engine" => s.engine = take(&mut i)?,
                    "--text" => s.text = Some(take(&mut i)?),
                    "--text-file" => s.text_file = Some(PathBuf::from(take(&mut i)?)),
                    "--out" => s.out = PathBuf::from(take(&mut i)?),
                    "--voice" => s.voice = Some(take(&mut i)?),
                    "--language" => s.language = Some(take(&mut i)?),
                    "--ref" => s.reference = Some(take(&mut i)?),
                    "--max-chars" => {
                        s.max_chars = Some(take(&mut i)?.parse().context("--max-chars must be a number")?)
                    }
                    "--param" => kv_pairs.push(take(&mut i)?),
                    other => bail!("unknown flag '{}'", other),
                }
                i += 1;
            }
            s.extra = parse_kv(&kv_pairs);
            if s.engine.is_empty() {
                bail!("--engine is required");
            }
            if s.out.as_os_str().is_empty() {
                bail!("--out is required");
            }
            let path = run_synthesize(s).await?;
            println!("{}", serde_json::json!({ "output": path.to_string_lossy() }));
            Ok(())
        }
        _ => {
            print_usage();
            std::process::exit(2);
        }
    }
}
