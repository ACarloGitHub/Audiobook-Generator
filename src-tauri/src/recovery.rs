//! Per-book recovery state.
//!
//! Written to `<book_dir>/failed_chunks.json` so an interrupted
//! generation can resume. See AudiobookGenerator-Wiki/wiki/concepts/plugin-architecture.md
//! for the recovery interaction with the engine lifecycle.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RecoveryState {
    pub done: HashMap<String, Vec<usize>>,
    pub failed: HashMap<String, Vec<FailedChunk>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FailedChunk {
    pub chunk_index: usize,
    pub text: String,
    pub error: String,
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
}
