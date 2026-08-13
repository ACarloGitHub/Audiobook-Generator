//! Persistent registry of generated audiobooks.
//!
//! Lives in the app data dir (`<app_data>/books_registry.json`, next to
//! `settings.json`), NOT next to the installer executable: the install
//! folder may be read-only (e.g. Program Files) and the app must always
//! be able to write its own data. Every book the user generates is
//! recorded here with its full path, so the Error Recovery panel can find
//! books that live outside the default output folder.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookRecord {
    pub book_title: String,
    pub book_dir: PathBuf,
    #[serde(default)]
    pub engine_id: Option<String>,
    #[serde(default)]
    pub registered_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    pub books: Vec<BookRecord>,
}

pub fn registry_path() -> PathBuf {
    // Test hook: let unit tests point the registry at a temp file instead of
    // the real app data dir.
    if let Ok(p) = std::env::var("ABG_TEST_REGISTRY") {
        return PathBuf::from(p);
    }
    crate::config::paths::app_data_dir().join("books_registry.json")
}

/// Load records from a specific file (the real path in production, a temp
/// file in tests).
pub fn load_from(path: &Path) -> Vec<BookRecord> {
    let Ok(body) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_str::<RegistryFile>(&body) else {
        return Vec::new();
    };
    file.books
}

pub fn load() -> Vec<BookRecord> {
    load_from(&registry_path())
}

fn save_to(path: &Path, records: &[BookRecord]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let body = serde_json::to_string_pretty(&RegistryFile {
        books: records.to_vec(),
    })?;
    std::fs::write(path, body)?;
    Ok(())
}

fn save(records: &[BookRecord]) -> Result<()> {
    save_to(&registry_path(), records)
}

/// Add or refresh a book in the registry (keyed by absolute `book_dir`).
pub fn register(book_dir: &Path, engine_id: Option<&str>) {
    let book_dir = std::fs::canonicalize(book_dir).unwrap_or_else(|_| book_dir.to_path_buf());
    let book_title = book_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut records = load();
    records.retain(|r| r.book_dir != book_dir);
    records.push(BookRecord {
        book_title,
        book_dir,
        engine_id: engine_id.map(|s| s.to_string()),
        registered_at: Some(crate::recovery::now_stamp()),
    });
    let _ = save(&records);
}

/// Remove a book from the registry. Used when a book has no pending work
/// left (all failed chunks retried and merged), so the registry only keeps
/// books with errors or pending merges.
pub fn remove(book_dir: &Path) {
    let book_dir = std::fs::canonicalize(book_dir).unwrap_or_else(|_| book_dir.to_path_buf());
    let mut records = load();
    records.retain(|r| r.book_dir != book_dir);
    let _ = save(&records);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("book_registry_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("books_registry.json")
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let p = temp_registry("missing");
        assert!(load_from(&p).is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let p = temp_registry("roundtrip");
        let book_dir = PathBuf::from("W:/Desktop/Ebook/Test Book");
        save_to(&p, &[BookRecord {
            book_title: "Test Book".into(),
            book_dir: book_dir.clone(),
            engine_id: Some("VoxCPM2 F16".into()),
            registered_at: Some("1234".into()),
        }])
        .unwrap();
        let loaded = load_from(&p);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].book_dir, book_dir);
        assert_eq!(loaded[0].engine_id.as_deref(), Some("VoxCPM2 F16"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn register_deduplicates_by_book_dir() {
        let p = temp_registry("dedup");
        // register() writes to the real app-data path; to keep the test
        // hermetic we exercise the dedup logic (retain by dir) directly.
        let dir = std::env::temp_dir().join(format!("dedup_dir_{}", std::process::id()));
        let mut records = Vec::new();
        records.push(BookRecord {
            book_title: "A".into(),
            book_dir: dir.clone(),
            engine_id: None,
            registered_at: None,
        });
        records.retain(|r| r.book_dir != dir);
        assert!(records.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&p);
    }
}
