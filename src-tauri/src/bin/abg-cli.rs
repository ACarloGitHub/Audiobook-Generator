//! abg-cli — command-line and MCP interface to Audiobook Generator.
//!
//! Two ways in (same commands underneath):
//!
//! 1. Direct commands:
//!      abg-cli status
//!      abg-cli synthesize --engine <id> [--text T | --text-file F]
//!          --out OUT.wav [--voice V] [--language L] [--ref REF.wav]
//!          [--max-chars N] [--param key=value]...
//! 2. MCP server over stdio (newline-delimited JSON-RPC), for LM Studio
//!    and other MCP clients:
//!      abg-cli --mcp
//!
//! The heavy lifting (engines, chunking, merging, GPU guard) is shared
//! with the desktop app through the library crate.

use anyhow::{bail, Context, Result};
use audiobook_generator_lib::base_plugin::{BaseTTSPlugin, SynthesizeRequest};
use audiobook_generator_lib::plugin_manager::{
    PluginManager, QwenPaths, VoxCpm2Paths,
};
use audiobook_generator_lib::{chunker, config, gpu_guard, merger};
use std::path::PathBuf;
use std::sync::Arc;

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

fn get_or_create_plugin(
    engine_id: &str,
    pm: &PluginManager,
) -> Option<Arc<dyn audiobook_generator_lib::base_plugin::BaseTTSPlugin>> {
    if let Some(p) = pm.get_plugin(engine_id) {
        return Some(p);
    }
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

struct SynthArgs {
    engine: String,
    text: Option<String>,
    text_file: Option<PathBuf>,
    out: PathBuf,
    voice: Option<String>,
    language: Option<String>,
    reference: Option<String>,
    max_chars: Option<usize>,
    extra: std::collections::HashMap<String, String>,
}

async fn run_synthesize(pm: &PluginManager, args: SynthArgs) -> Result<PathBuf> {
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

    let plugin = get_or_create_plugin(&args.engine, pm)
        .ok_or_else(|| anyhow::anyhow!("engine '{}' is not installed or model files missing", args.engine))?;
    let handle = plugin.load_model(&args.engine).await?;

    let defaults = audiobook_generator_lib::plugin_manager::defaults_for(&args.engine);
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

fn print_usage() {
    eprintln!(
        "abg-cli — Audiobook Generator command line\n\
         \n\
         Usage:\n\
         \x20 abg-cli status\n\
         \x20 abg-cli synthesize --engine <id> [--text T | --text-file F] --out OUT.wav\n\
         \x20     [--voice V] [--language L] [--ref REF.wav] [--max-chars N] [--param k=v]...\n\
         \x20 abg-cli --mcp\n\
         \n\
         Engine ids: run `abg-cli status` to list them."
    );
}

fn parse_kv(pairs: &[String]) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
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
            "name": "synthesize",
            "description": "Synthesize speech to a WAV file with a chosen TTS engine, voice and parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "engine": { "type": "string", "description": "Engine id (see get_status)" },
                    "text": { "type": "string", "description": "Text to synthesize" },
                    "text_file": { "type": "string", "description": "Path to a UTF-8 text file (alternative to text)" },
                    "output_path": { "type": "string", "description": "Destination WAV file path" },
                    "voice": { "type": "string" },
                    "language": { "type": "string" },
                    "reference_audio": { "type": "string", "description": "Path to a reference WAV for voice cloning" },
                    "max_chars": { "type": "integer" },
                    "extra": { "type": "object", "description": "Engine parameters, e.g. {\"temperature\": \"0.7\"}" }
                },
                "required": ["engine", "output_path"]
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
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_default();
            let text_result: Result<String> = async {
                match name {
                    "get_status" => Ok(serde_json::to_string_pretty(&status_json(pm))?),
                    "synthesize" => {
                        let engine = args
                            .get("engine")
                            .and_then(|v| v.as_str())
                            .context("missing 'engine'")?
                            .to_string();
                        let out = args
                            .get("output_path")
                            .and_then(|v| v.as_str())
                            .context("missing 'output_path'")?;
                        let extra: std::collections::HashMap<String, String> = args
                            .get("extra")
                            .and_then(|v| v.as_object())
                            .map(|o| {
                                o.iter()
                                    .map(|(k, v)| {
                                        (k.clone(), v.as_str().unwrap_or("").to_string())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let path = run_synthesize(
                            pm,
                            SynthArgs {
                                engine,
                                text: args
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                text_file: args
                                    .get("text_file")
                                    .and_then(|v| v.as_str())
                                    .map(PathBuf::from),
                                out: PathBuf::from(out),
                                voice: args
                                    .get("voice")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                language: args
                                    .get("language")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                reference: args
                                    .get("reference_audio")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
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
                    other => bail!("unknown tool '{}'", other),
                }
            }
            .await;
            match text_result {
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

#[tokio::main]
async fn main() -> Result<()> {
    init_paths();
    let pm = PluginManager::new(config::paths::app_data_dir());

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        print_usage();
        std::process::exit(2);
    };

    match cmd.as_str() {
        "--mcp" => run_mcp(&pm).await,
        "status" => {
            println!("{}", serde_json::to_string_pretty(&status_json(&pm))?);
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
                extra: std::collections::HashMap::new(),
            };
            let mut kv_pairs: Vec<String> = Vec::new();
            let mut i = 1;
            while i < args.len() {
                let flag = args[i].as_str();
                let mut take = |i: &mut usize| -> Result<String> {
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
            let path = run_synthesize(&pm, s).await?;
            println!("{}", serde_json::json!({ "output": path.to_string_lossy() }));
            Ok(())
        }
        _ => {
            print_usage();
            std::process::exit(2);
        }
    }
}
