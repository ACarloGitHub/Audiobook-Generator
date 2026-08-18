//! Persistent history of every generation job (A7).
//!
//! Lives in the app data dir (`<app_data>/jobs_history.json`), written only
//! by Audiobook Generator. Unlike the recovery state (`failed_chunks.json`,
//! deleted once a book has no pending work) and the recovery registry
//! (`books_registry.json`, only books with pending work), the history keeps
//! EVERY job — including completed ones — so the user can resume or restart
//! it from the History / AuraWrite panels even when the output folder was
//! deleted (a job whose output folder is gone is shown as "missing" and can
//! be continued with the folder recreated from scratch).
//!
//! The record is keyed by the absolute output folder path (`book_dir`).
//! For books coming from AuraWrite the `aurawrite_book_id` links the job to
//! the unified ebook catalog.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    #[serde(default)]
    pub aurawrite_book_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub source_document: Option<String>,
    pub book_dir: PathBuf,
    #[serde(default)]
    pub engine_id: Option<String>,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub reference_audio: Option<String>,
    #[serde(default)]
    pub reference_transcript: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, String>,
    /// Cumulative list of chapter titles already converted (across sessions).
    #[serde(default)]
    pub converted_chapters: Vec<String>,
    /// Full chapter list of the source document (captured at generation
    /// start), used to compute "chapters remaining" and "completed".
    #[serde(default)]
    pub chapter_titles: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    pub jobs: Vec<JobRecord>,
}

/// Where the history file lives. Overridden in tests via `ABG_TEST_HISTORY`.
pub fn history_path() -> PathBuf {
    if let Ok(p) = std::env::var("ABG_TEST_HISTORY") {
        return PathBuf::from(p);
    }
    crate::config::paths::app_data_dir().join("jobs_history.json")
}

pub fn load() -> Vec<JobRecord> {
    let Ok(body) = std::fs::read_to_string(history_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<HistoryFile>(&body)
        .map(|f| f.jobs)
        .unwrap_or_default()
}

fn save(records: &[JobRecord]) -> Result<(), String> {
    let path = history_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(&HistoryFile {
        jobs: records.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())
}

/// Settings captured when a generation starts, used to create/refresh the
/// job record.
pub struct GenerationSettings<'a> {
    pub book_dir: &'a Path,
    pub source_document: Option<&'a str>,
    pub title: &'a str,
    pub engine_id: Option<&'a str>,
    pub voice: Option<&'a str>,
    pub language: Option<&'a str>,
    pub reference_audio: Option<&'a str>,
    pub reference_transcript: Option<&'a str>,
    pub params: HashMap<String, String>,
    pub chapter_titles: Vec<String>,
    pub aurawrite_book_id: Option<String>,
}

/// Create or refresh the job record for a generation that is starting.
pub fn note_generation_started(s: GenerationSettings) {
    let mut records = load();
    let book_dir = s.book_dir.to_path_buf();
    if let Some(r) = records.iter_mut().find(|r| r.book_dir == book_dir) {
        if let Some(v) = s.engine_id {
            r.engine_id = Some(v.to_string());
        }
        if let Some(v) = s.voice {
            r.voice = Some(v.to_string());
        }
        if let Some(v) = s.language {
            r.language = Some(v.to_string());
        }
        if let Some(v) = s.reference_audio {
            r.reference_audio = Some(v.to_string());
        }
        if let Some(v) = s.reference_transcript {
            r.reference_transcript = Some(v.to_string());
        }
        if let Some(v) = s.source_document {
            r.source_document = Some(v.to_string());
        }
        if !s.params.is_empty() {
            r.params = s.params;
        }
        if !s.chapter_titles.is_empty() {
            r.chapter_titles = s.chapter_titles;
        }
        if let Some(v) = s.aurawrite_book_id {
            r.aurawrite_book_id = Some(v);
        }
        r.updated_at = Some(crate::recovery::now_stamp());
    } else {
        let now = crate::recovery::now_stamp();
        let title = if s.title.is_empty() {
            book_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            s.title.to_string()
        };
        records.push(JobRecord {
            id: book_dir.to_string_lossy().into_owned(),
            aurawrite_book_id: s.aurawrite_book_id,
            title,
            source_document: s.source_document.map(|x| x.to_string()),
            book_dir,
            engine_id: s.engine_id.map(|x| x.to_string()),
            voice: s.voice.map(|x| x.to_string()),
            language: s.language.map(|x| x.to_string()),
            reference_audio: s.reference_audio.map(|x| x.to_string()),
            reference_transcript: s.reference_transcript.map(|x| x.to_string()),
            params: s.params,
            converted_chapters: Vec::new(),
            chapter_titles: s.chapter_titles,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            completed_at: None,
        });
    }
    let _ = save(&records);
}

fn mp3_exists(book_dir: &Path, title: &str) -> bool {
    book_dir
        .join(format!("{}.mp3", crate::utils::sanitize_filename(title)))
        .exists()
}

fn update_completion(r: &mut JobRecord, book_dir: &Path) {
    if !r.chapter_titles.is_empty()
        && r.chapter_titles
            .iter()
            .all(|t| r.converted_chapters.contains(t) || mp3_exists(book_dir, t))
    {
        if r.completed_at.is_none() {
            r.completed_at = Some(crate::recovery::now_stamp());
        }
    } else {
        r.completed_at = None;
    }
}

/// After a generation (success or failure), recompute the converted chapters
/// from the MP3 files present in the output folder and refresh the status.
pub fn sync_converted_from_disk(book_dir: &Path) {
    let mut records = load();
    let Some(r) = records.iter_mut().find(|r| r.book_dir == book_dir) else {
        return;
    };
    let mut titles: HashSet<String> = r.converted_chapters.iter().cloned().collect();
    if !r.chapter_titles.is_empty() {
        for t in &r.chapter_titles {
            if mp3_exists(book_dir, t) {
                titles.insert(t.clone());
            }
        }
    }
    let mut sorted: Vec<String> = titles.into_iter().collect();
    sorted.sort();
    r.converted_chapters = sorted;
    update_completion(r, book_dir);
    r.updated_at = Some(crate::recovery::now_stamp());
    let _ = save(&records);
}

/// Recreate the output folder (Continue / Restart for a missing folder).
pub fn ensure_dir(book_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(book_dir).map_err(|e| e.to_string())
}

/// Public view of a job record with derived fields for the UI.
#[derive(Serialize)]
pub struct HistoryJobView {
    pub id: String,
    pub aurawrite_book_id: Option<String>,
    pub title: String,
    pub source_document: Option<String>,
    pub book_dir: String,
    pub engine_id: Option<String>,
    pub voice: Option<String>,
    pub language: Option<String>,
    pub reference_audio: Option<String>,
    pub reference_transcript: Option<String>,
    pub params: HashMap<String, String>,
    pub converted_chapters: Vec<String>,
    pub chapter_titles: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    /// Whether the output folder currently exists on disk.
    pub dir_exists: bool,
    /// "missing" (folder gone) | "completed" | "in_progress".
    pub status: String,
    pub converted_count: usize,
    pub total_count: usize,
}

fn to_view(r: &JobRecord) -> HistoryJobView {
    let dir_exists = r.book_dir.is_dir();
    let status = if !dir_exists {
        "missing".to_string()
    } else if r.completed_at.is_some() {
        "completed".to_string()
    } else {
        "in_progress".to_string()
    };
    HistoryJobView {
        id: r.id.clone(),
        aurawrite_book_id: r.aurawrite_book_id.clone(),
        title: r.title.clone(),
        source_document: r.source_document.clone(),
        book_dir: r.book_dir.to_string_lossy().into_owned(),
        engine_id: r.engine_id.clone(),
        voice: r.voice.clone(),
        language: r.language.clone(),
        reference_audio: r.reference_audio.clone(),
        reference_transcript: r.reference_transcript.clone(),
        params: r.params.clone(),
        converted_chapters: r.converted_chapters.clone(),
        chapter_titles: r.chapter_titles.clone(),
        created_at: r.created_at.clone(),
        updated_at: r.updated_at.clone(),
        completed_at: r.completed_at.clone(),
        dir_exists,
        status,
        converted_count: r.converted_chapters.len(),
        total_count: r.chapter_titles.len(),
    }
}

/// All jobs, newest first, with derived status for the History panel.
pub fn list_views() -> Vec<HistoryJobView> {
    let mut records = load();
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    records.iter().map(to_view).collect()
}

/// The view of a single job keyed by its output folder.
pub fn get_view(book_dir: &Path) -> Option<HistoryJobView> {
    load().iter().find(|r| r.book_dir == book_dir).map(to_view)
}

#[tauri::command]
pub fn history_list() -> Vec<HistoryJobView> {
    list_views()
}

#[tauri::command]
pub fn history_open(book_dir: String) -> Result<HistoryJobView, String> {
    let path = PathBuf::from(book_dir);
    // Recompute the converted chapters from the MP3 files actually present
    // (covers jobs created before this feature / manually moved files).
    if path.is_dir() {
        sync_converted_from_disk(&path);
    }
    get_view(&path).ok_or_else(|| "The job is not in the history.".to_string())
}

#[tauri::command]
pub fn history_continue(book_dir: String) -> Result<HistoryJobView, String> {
    let path = PathBuf::from(&book_dir);
    ensure_dir(&path).map_err(|e| format!("recreate the output folder: {e}"))?;
    get_view(&path).ok_or_else(|| "The job is not in the history.".to_string())
}

#[tauri::command]
pub fn history_restart(book_dir: String) -> Result<HistoryJobView, String> {
    let path = PathBuf::from(&book_dir);
    ensure_dir(&path).map_err(|e| format!("recreate the output folder: {e}"))?;
    get_view(&path).ok_or_else(|| "The job is not in the history.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests in this module swap the global ABG_TEST_HISTORY env var; a mutex
    // serializes them so they cannot clobber each other while running in
    // parallel.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_history(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("history_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("jobs_history.json")
    }

    fn settings(book_dir: &Path) -> GenerationSettings<'_> {
        GenerationSettings {
            book_dir,
            source_document: Some("W:/Books/mybook.epub"),
            title: "My Book",
            engine_id: Some("Qwen3-TTS-12Hz-0.6B-CustomVoice"),
            voice: Some("Serena"),
            language: Some("Italian"),
            reference_audio: Some("W:/Ref/ref.wav"),
            reference_transcript: None,
            params: HashMap::new(),
            chapter_titles: vec![
                "Chapter 1".into(),
                "Chapter 2".into(),
                "Chapter 3".into(),
            ],
            aurawrite_book_id: Some("book-1".into()),
        }
    }

    #[test]
    fn note_creates_and_updates_record() {
        let _guard = TEST_LOCK.lock().unwrap();
        let p = temp_history("note");
        std::env::set_var("ABG_TEST_HISTORY", &p);
        let dir = std::env::temp_dir().join(format!("history_note_dir_{}", std::process::id()));
        note_generation_started(settings(&dir));
        note_generation_started(settings(&dir));
        let records = load();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].aurawrite_book_id.as_deref(), Some("book-1"));
        assert_eq!(records[0].converted_chapters.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("ABG_TEST_HISTORY");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn sync_adds_converted_from_mp3() {
        let _guard = TEST_LOCK.lock().unwrap();
        let p = temp_history("sync");
        std::env::set_var("ABG_TEST_HISTORY", &p);
        let dir = std::env::temp_dir().join(format!("history_sync_dir_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Chapter_1.mp3"), b"x").unwrap();
        std::fs::write(dir.join("Chapter_2.mp3"), b"x").unwrap();
        note_generation_started(settings(&dir));
        sync_converted_from_disk(&dir);
        let records = load();
        assert_eq!(records[0].converted_chapters, vec!["Chapter 1".to_string(), "Chapter 2".to_string()]);
        assert!(records[0].completed_at.is_none());
        std::fs::write(dir.join("Chapter_3.mp3"), b"x").unwrap();
        sync_converted_from_disk(&dir);
        let records = load();
        assert!(records[0].completed_at.is_some());
        assert_eq!(to_view(&records[0]).status, "completed");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("ABG_TEST_HISTORY");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn view_reports_missing_when_dir_gone() {
        let _guard = TEST_LOCK.lock().unwrap();
        let p = temp_history("missing");
        std::env::set_var("ABG_TEST_HISTORY", &p);
        let dir = std::env::temp_dir().join(format!("history_missing_dir_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        note_generation_started(settings(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        let view = get_view(&dir).unwrap();
        assert_eq!(view.status, "missing");
        assert!(!view.dir_exists);
        std::env::remove_var("ABG_TEST_HISTORY");
        let _ = std::fs::remove_file(&p);
    }
}
