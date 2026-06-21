//! Text chunking for TTS.
//!
//! The strategy mirrors the legacy Python `chunk_chapter_text`: split on
//! sentence boundaries, greedily group sentences into chunks of at most
//! `max_words` words, and hard-split anything that is still over the
//! character budget (Kokoro's 510-token input limit) on word boundaries.

use regex::Regex;
use std::sync::OnceLock;

/// Kokoro-82M accepts at most 510 input tokens per synthesis call.
pub const MAX_TOKENS: usize = 510;
/// ~3 characters per token is a safe upper bound for English /
/// Italian / French text after the Kokoro tokenizer encodes it.
pub const CHARS_PER_TOKEN: usize = 3;
pub const MAX_CHARS_PER_CHUNK: usize = MAX_TOKENS * CHARS_PER_TOKEN;

/// Split a chapter into chunks suitable for TTS.
pub fn chunk_text(text: &str, max_words: usize) -> Vec<String> {
    let sentences = split_sentences(text);
    let mut chunks: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_words: usize = 0;
    let mut current_chars: usize = 0;

    for sentence in sentences {
        let trimmed = sentence.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let words = trimmed.split_whitespace().count();
        let chars = trimmed.chars().count();

        let would_exceed_words = current_words + words > max_words && !current.is_empty();
        let would_exceed_chars = current_chars + chars + 1 > MAX_CHARS_PER_CHUNK && !current.is_empty();

        if would_exceed_words || would_exceed_chars {
            chunks.push(current.join(" "));
            current.clear();
            current_words = 0;
            current_chars = 0;
        }
        current.push(trimmed);
        current_words += words;
        current_chars += chars + 1;
    }

    if !current.is_empty() {
        chunks.push(current.join(" "));
    }

    let mut final_chunks: Vec<String> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        if chunk.chars().count() <= MAX_CHARS_PER_CHUNK {
            final_chunks.push(chunk);
        } else {
            final_chunks.extend(hard_split(&chunk, MAX_CHARS_PER_CHUNK));
        }
    }

    if final_chunks.is_empty() {
        final_chunks.push(text.trim().to_string());
    }

    final_chunks
}

fn hard_split(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.chars().count() + word.chars().count() + 1 > max_chars && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Cheap regex-based sentence splitter. Good enough for the prototype;
/// the real core can swap in `sentencex` with the right API once we pin
/// a stable version.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?' | '\n') {
            let mut next_idx = i + c.len_utf8();
            while next_idx < bytes.len() && bytes[next_idx] == b' ' {
                next_idx += 1;
            }
            if next_idx < bytes.len() && bytes[next_idx].is_ascii_alphabetic() {
                continue;
            }
            out.push(text[start..i + c.len_utf8()].to_string());
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        out.push(text[start..].to_string());
    }
    out
}

// Silence the unused-import warning for `regex` if we end up not using
// it in the future; for now we use a manual splitter. Kept here as a
// placeholder for the upgrade to a real sentencex-style splitter.
#[allow(dead_code)]
fn regex_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[.!?]\s+").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_yields_one_chunk() {
        let out = chunk_text("", 50);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], "");
    }

    #[test]
    fn groups_short_sentences() {
        let text = "One. Two. Three. Four. Five.";
        let out = chunk_text(text, 50);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("One."));
    }

    #[test]
    fn splits_when_exceeding_max() {
        let text = "Alpha bravo charlie. Delta echo foxtrot. Golf hotel india. Juliett kilo lima.";
        let out = chunk_text(text, 4);
        assert!(out.len() >= 2);
    }

    #[test]
    fn preserves_long_sentence() {
        let long = "a ".repeat(100);
        let out = chunk_text(&long, 20);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn does_not_split_on_common_abbreviations() {
        let text = "Dr. Smith went to the lab. He discovered something.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2);
        assert!(out[0].starts_with("Dr."));
    }
}
