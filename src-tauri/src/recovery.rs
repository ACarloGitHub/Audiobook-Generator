//! Per-book recovery state.
//!
//! Written to `<book_dir>/failed_chunks.json` so an interrupted
//! generation can resume. See AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md
//! for the recovery interaction with the engine lifecycle.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Generation metadata captured when a book is synthesized, so the recovery
/// commands can re-synthesize failed chunks with the same engine and
/// parameters. All fields are optional: recovery files written before this
/// struct existed deserialize with `None` everywhere, and the retry commands
/// fall back to the currently selected engine in that case.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RecoveryMeta {
    #[serde(default)]
    pub engine_id: Option<String>,
    #[serde(default)]
    pub reference_audio: Option<String>,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, String>,
    #[serde(default)]
    pub generated_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RecoveryState {
    #[serde(default)]
    pub meta: RecoveryMeta,
    pub done: HashMap<String, Vec<usize>>,
    pub failed: HashMap<String, Vec<FailedChunk>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedChunk {
    pub chunk_index: usize,
    pub text: String,
    pub error: String,
}

/// Timestamp string (seconds since UNIX epoch) for recovery metadata.
/// Kept dependency-free; it only needs to be informative, not pretty.
pub fn now_stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

impl RecoveryState {
    pub fn load(book_dir: &Path) -> Result<Self> {
        let path = book_dir.join("failed_chunks.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let body = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&body).unwrap_or_default())
    }

    pub fn save(&self, book_dir: &Path) -> Result<()> {
        let path = book_dir.join("failed_chunks.json");
        let body = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, body)?;
        Ok(())
    }

    /// Delete `failed_chunks.json` when there is nothing left to recover.
    pub fn remove_file_if_empty(book_dir: &Path, state: &Self) -> Result<()> {
        if state.failed.is_empty() && state.done.is_empty() {
            let path = book_dir.join("failed_chunks.json");
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
        Ok(())
    }

    pub fn set_meta(&mut self, meta: RecoveryMeta) {
        self.meta = meta;
    }

    pub fn is_done(&self, chapter: &str, chunk_index: usize) -> bool {
        self.done
            .get(chapter)
            .map(|v| v.contains(&chunk_index))
            .unwrap_or(false)
    }

    pub fn mark_done(&mut self, chapter: &str, chunk_index: usize) {
        self.done
            .entry(chapter.to_string())
            .or_default()
            .push(chunk_index);
    }

    pub fn mark_failed(&mut self, chapter: &str, chunk_index: usize, text: &str, error: &str) {
        self.failed
            .entry(chapter.to_string())
            .or_default()
            .push(FailedChunk {
                chunk_index,
                text: text.to_string(),
                error: error.to_string(),
            });
    }

    /// Remove a failed-chunk record (e.g. after a successful retry) and
    /// drop the chapter entry from `failed` once it has no failures left.
    pub fn remove_failed(&mut self, chapter: &str, chunk_index: usize) {
        if let Some(v) = self.failed.get_mut(chapter) {
            v.retain(|f| f.chunk_index != chunk_index);
            if v.is_empty() {
                self.failed.remove(chapter);
            }
        }
    }

    /// Update the error (and optionally the text) of an existing failed
    /// record. If the record does not exist, it is created.
    pub fn update_failed(&mut self, chapter: &str, chunk_index: usize, text: &str, error: &str) {
        let entry = self.failed.entry(chapter.to_string()).or_default();
        if let Some(f) = entry.iter_mut().find(|f| f.chunk_index == chunk_index) {
            f.text = text.to_string();
            f.error = error.to_string();
        } else {
            entry.push(FailedChunk {
                chunk_index,
                text: text.to_string(),
                error: error.to_string(),
            });
        }
    }

    /// Drop a chapter from both `done` and `failed` (after a manual merge
    /// the chapter is considered finished and no longer tracked).
    pub fn clear_chapter(&mut self, chapter: &str) {
        self.done.remove(chapter);
        self.failed.remove(chapter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_json_without_meta_deserializes() {
        let body = r#"{"done":{"Cap 1":[0,1]},"failed":{"Cap 1":[{"chunk_index":2,"text":"abc","error":"boom"}]}}"#;
        let state: RecoveryState = serde_json::from_str(body).unwrap();
        assert!(state.meta.engine_id.is_none());
        assert!(state.meta.extra.is_empty());
        assert_eq!(state.done["Cap 1"], vec![0, 1]);
        assert_eq!(state.failed["Cap 1"][0].chunk_index, 2);
    }

    #[test]
    fn remove_failed_drops_empty_chapter_entry() {
        let mut state = RecoveryState::default();
        state.mark_failed("Cap 1", 3, "txt", "err");
        state.mark_failed("Cap 1", 4, "txt", "err");
        state.remove_failed("Cap 1", 3);
        assert_eq!(state.failed["Cap 1"].len(), 1);
        state.remove_failed("Cap 1", 4);
        assert!(!state.failed.contains_key("Cap 1"));
    }

    #[test]
    fn update_failed_edits_existing_record() {
        let mut state = RecoveryState::default();
        state.mark_failed("Cap 1", 0, "old text", "old error");
        state.update_failed("Cap 1", 0, "new text", "new error");
        let f = &state.failed["Cap 1"][0];
        assert_eq!(f.text, "new text");
        assert_eq!(f.error, "new error");
        assert_eq!(state.failed["Cap 1"].len(), 1);
    }

    #[test]
    fn clear_chapter_removes_done_and_failed() {
        let mut state = RecoveryState::default();
        state.mark_done("Cap 1", 0);
        state.mark_failed("Cap 1", 1, "t", "e");
        state.mark_done("Cap 2", 0);
        state.clear_chapter("Cap 1");
        assert!(!state.done.contains_key("Cap 1"));
        assert!(!state.failed.contains_key("Cap 1"));
        assert!(state.done.contains_key("Cap 2"));
    }

    #[test]
    fn save_and_load_roundtrip_with_meta() {
        let dir = std::env::temp_dir().join(format!("recovery_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = RecoveryState::default();
        let mut extra = HashMap::new();
        extra.insert("temp".to_string(), "0.7".to_string());
        state.set_meta(RecoveryMeta {
            engine_id: Some("Qwen3-TTS-12Hz-0.6B-Base".to_string()),
            reference_audio: Some("ref.wav".to_string()),
            voice: None,
            language: Some("Italian".to_string()),
            extra,
            generated_at: Some("2026-07-20T10:00:00Z".to_string()),
        });
        state.mark_failed("Cap 1", 5, "hello", "boom");
        state.save(&dir).unwrap();
        let loaded = RecoveryState::load(&dir).unwrap();
        assert_eq!(
            loaded.meta.engine_id.as_deref(),
            Some("Qwen3-TTS-12Hz-0.6B-Base")
        );
        assert_eq!(loaded.meta.extra.get("temp").map(|s| s.as_str()), Some("0.7"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
