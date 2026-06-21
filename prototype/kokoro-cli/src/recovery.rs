use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persistent record of which chunks have completed across runs.
///
/// Stored as `<output_dir>/failed_chunks.json` so the user can inspect it
/// and so a future run can resume from where the last one stopped.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RecoveryState {
    /// Map of `chapter_title -> set of chunk indices that completed`.
    pub done: HashMap<String, Vec<usize>>,
    /// Map of `chapter_title -> list of failed chunks with diagnostics`.
    pub failed: HashMap<String, Vec<FailedChunk>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FailedChunk {
    pub chunk_index: usize,
    pub text: String,
    pub error: String,
}

impl RecoveryState {
    pub fn load(output_dir: &Path) -> Self {
        let path = output_dir.join("failed_chunks.json");
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(e) => {
                tracing::warn!(
                    "failed to read {}: {e}; starting with empty recovery state",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn is_done(&self, chapter: &str, chunk_index: usize) -> bool {
        self.done
            .get(chapter)
            .map(|v| v.contains(&chunk_index))
            .unwrap_or(false)
    }

    pub fn mark_done(&mut self, chapter: &str, chunk_index: usize) {
        self.done.entry(chapter.to_string()).or_default().push(chunk_index);
        let _ = self.persist_to_default_location();
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
        let _ = self.persist_to_default_location();
    }

    /// We persist into the current working directory's `failed_chunks.json`
    /// for the prototype. The real core will use a stable per-book
    /// path inside the book output directory.
    fn persist_to_default_location(&self) -> Result<()> {
        let path = default_recovery_path();
        let body = serde_json::to_string_pretty(self).context("failed to serialise recovery state")?;
        std::fs::write(&path, body)
            .with_context(|| format!("failed to write recovery state to {}", path.display()))?;
        Ok(())
    }
}

fn default_recovery_path() -> PathBuf {
    PathBuf::from("failed_chunks.json")
}
