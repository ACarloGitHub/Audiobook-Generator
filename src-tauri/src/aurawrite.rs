// Integration with the external app "AuraWrite" (the twin of this integration).
//
// Audiobook Generator reads the handover that AuraWrite writes into this
// app's data dir:
//   - the proposal ("campanello"): `aurawrite-proposal.json` with the ebook
//     path. It is consumed (deleted) after being read, so it does not reapply
//     on the next launch.
//   - the visit card: `aurawrite-visit-card.json`, pointing to AuraWrite's
//     data dir and to the unified ebook catalog.
//   - the catalog: `aurawrite-ebooks.json`, published by AuraWrite
//     (Editor + Reader), read in read-only mode.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const AUDIOBOOK_DATA_DIR: &str = "com.patata.audiobookgenerator";
const AURAWRITE_PROPOSAL: &str = "aurawrite-proposal.json";
const AURAWRITE_VISIT_CARD: &str = "aurawrite-visit-card.json";
const AURAWRITE_CATALOG: &str = "aurawrite-ebooks.json";

#[derive(Serialize)]
pub struct AurawriteCheck {
    pub found_aurawrite: bool,
    pub has_proposal: bool,
}

#[derive(Serialize)]
pub struct AurawriteProposal {
    pub input: String,
}

fn audiobook_data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join(AUDIOBOOK_DATA_DIR))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_json(path: &std::path::Path) -> Option<Value> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn visit_card() -> Option<Value> {
    read_json(&audiobook_data_dir().join(AURAWRITE_VISIT_CARD))
}

/// Locate AuraWrite's data dir: the visit card first (exact path), then the
/// standard per-platform location (Windows: %APPDATA%\com.aurawrite.desktop).
fn find_aurawrite_data_dir() -> Option<PathBuf> {
    if let Some(card) = visit_card() {
        if let Some(dir) = card.get("aurawrite_data_dir").and_then(|v| v.as_str()) {
            let p = PathBuf::from(dir);
            if p.exists() {
                return Some(p);
            }
        }
    }
    let standard = dirs::data_dir()?.join("com.aurawrite.desktop");
    if standard.exists() {
        return Some(standard);
    }
    None
}

/// Read and consume the proposal file, if any.
fn take_proposal_file() -> Option<AurawriteProposal> {
    let path = audiobook_data_dir().join(AURAWRITE_PROPOSAL);
    let value = read_json(&path)?;
    let _ = fs::remove_file(&path);
    let input = value.get("input").and_then(|v| v.as_str())?.to_string();
    Some(AurawriteProposal { input })
}

/// Read the unified ebook catalog published by AuraWrite.
fn read_catalog() -> Option<Value> {
    if let Some(p) = visit_card()
        .and_then(|c| c.get("catalog").and_then(|v| v.as_str()).map(PathBuf::from))
    {
        if p.exists() {
            return read_json(&p);
        }
    }
    let dir = find_aurawrite_data_dir()?;
    read_json(&dir.join(AURAWRITE_CATALOG))
}

/// Whether AuraWrite is reachable and a proposal is waiting.
#[tauri::command]
pub fn aurawrite_check() -> Result<AurawriteCheck, String> {
    let found_aurawrite = find_aurawrite_data_dir().is_some();
    let has_proposal = audiobook_data_dir().join(AURAWRITE_PROPOSAL).exists();
    Ok(AurawriteCheck {
        found_aurawrite,
        has_proposal,
    })
}

/// Take (read + consume) the pending proposal, if any.
#[tauri::command]
pub fn aurawrite_take_proposal() -> Result<Option<AurawriteProposal>, String> {
    Ok(take_proposal_file())
}

/// The unified ebook catalog published by AuraWrite (read-only).
#[tauri::command]
pub fn aurawrite_catalog() -> Result<Option<Value>, String> {
    Ok(read_catalog())
}

