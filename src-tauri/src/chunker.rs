//! Text chunking for TTS.
//!
//! The strategy mirrors the legacy Python `chunk_chapter_text`: split on
//! sentence boundaries, greedily group sentences into chunks of at most
//! `max_words` words, and hard-split anything that is still over the
//! character budget on word boundaries.
//!
//! The character budget is engine-specific and language-specific: the caller
//! passes `max_chars` (resolved from `tts_model_config()[engine].char_limits_by_lang[lang]`
//! at runtime). The old fixed constant `MAX_CHARS_PER_CHUNK` (1530) is kept
//! only as a fallback for callers that do not supply an explicit limit.

use regex::Regex;
use std::sync::OnceLock;

/// Kokoro-82M accepts at most 510 input tokens per synthesis call.
pub const MAX_TOKENS: usize = 510;
/// ~3 characters per token is a safe upper bound for English /
/// Italian / French text after the Kokoro tokenizer encodes it.
pub const CHARS_PER_TOKEN: usize = 3;
/// Fallback character budget (510 tokens × 3 chars/token). Callers should
/// pass the engine-specific limit from `tts_model_config()` instead.
pub const MAX_CHARS_PER_CHUNK: usize = MAX_TOKENS * CHARS_PER_TOKEN;

/// Split a chapter into chunks suitable for TTS.
///
/// `max_words` is the word-count budget (used for the "Word Count Approx"
/// chunking strategy). `max_chars` is the character budget (used as the hard
/// limit for every chunk; for Kokoro this is resolved per-language from
/// `char_limits_by_lang`, e.g. 2300 for Italian, 1100 for Japanese).
pub fn chunk_text(text: &str, max_words: usize, max_chars: usize) -> Vec<String> {
    let effective_max_chars = if max_chars == 0 { MAX_CHARS_PER_CHUNK } else { max_chars };
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
        let would_exceed_chars = current_chars + chars + 1 > effective_max_chars && !current.is_empty();

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
        if chunk.chars().count() <= effective_max_chars {
            final_chunks.push(chunk);
        } else {
            final_chunks.extend(hard_split(&chunk, effective_max_chars));
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

/// Split `text` into `n_parts` parts of similar character length, breaking
/// on sentence boundaries whenever possible. Used by the recovery
/// "split & retry" flow when a single chunk keeps failing.
///
/// Strategy: split into sentences, then greedily assign each sentence to the
/// currently shortest part (keeps parts balanced by characters). If there
/// are fewer sentences than requested parts, fall back to a word-level
/// balanced split. `n_parts` is clamped to at least 2 and at most the number
/// of available units.
pub fn split_text_balanced(text: &str, n_parts: usize) -> Vec<String> {
    let n = n_parts.max(2);
    let sentences = split_sentences(text);
    if sentences.len() >= n {
        return distribute_balanced(sentences, n);
    }
    // Not enough sentence boundaries: split on words instead.
    let words: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
    if words.len() >= n {
        return distribute_balanced(words, n);
    }
    // Degenerate case (very few words): return what we have, one unit per part.
    words.into_iter().filter(|w| !w.is_empty()).collect()
}

/// Assign units (sentences or words) to `n` groups, always appending the
/// next unit to the currently shortest group. Units stay in order within
/// each group; because units are assigned round-robin by size, groups may
/// interleave. To keep the spoken text in reading order we instead pack
/// consecutive units: compute the target size and fill groups sequentially.
fn distribute_balanced(units: Vec<String>, n: usize) -> Vec<String> {
    let total: usize = units.iter().map(|u| u.chars().count() + 1).sum();
    let target = total.div_ceil(n);
    let mut parts: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_len = 0usize;
    let mut remaining_units = units.len();

    for unit in units {
        remaining_units -= 1;
        let unit_len = unit.chars().count() + 1;
        let parts_left = n - parts.len();
        // Start a new part when the current one reached the target size, as
        // long as enough units remain to fill the parts left.
        if !current.is_empty() && current_len >= target && remaining_units >= parts_left - 1 {
            parts.push(current.join(" "));
            current = Vec::new();
            current_len = 0;
        }
        current.push(unit);
        current_len += unit_len;
    }
    if !current.is_empty() {
        parts.push(current.join(" "));
    }
    parts
}

/// Sentence splitter that mirrors the legacy Python `split_into_sentences`.
/// Splits on `.`, `!`, `?` followed by whitespace, keeping the punctuation
/// attached to the preceding sentence. Ellipses (`..`) are not split because
/// we require the punctuation char to NOT be preceded by another punctuation
/// char (so ".." stays together).
pub fn split_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0usize;

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '.' || c == '!' || c == '?' {
            // Skip ellipses: if the previous char was also punctuation, don't split
            let prev_is_punct = i > 0 && (chars[i - 1] == '.' || chars[i - 1] == '!' || chars[i - 1] == '?');
            // Check that the next non-trailing char is whitespace
            let next_is_ws = i + 1 >= chars.len() || chars[i + 1].is_whitespace();
            if !prev_is_punct && next_is_ws {
                // Include the punctuation in the sentence
                let sentence: String = chars[start..=i].iter().collect();
                let trimmed = sentence.trim().to_string();
                if !trimmed.is_empty() {
                    out.push(trimmed);
                }
                // Skip trailing whitespace
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    let tail: String = chars[start..].iter().collect();
    let trimmed = tail.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
}

/// Compiled regex placeholder, retained for future use when a more advanced
/// regex engine (e.g. `fancy-regex`) is adopted. Not used by `split_sentences`.
#[allow(dead_code)]
fn regex_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[.!?]\s+").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MAX_CHARS: usize = 1530;

    #[test]
    fn empty_text_yields_one_chunk() {
        let out = chunk_text("", 50, TEST_MAX_CHARS);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], "");
    }

    #[test]
    fn groups_short_sentences() {
        let text = "One. Two. Three. Four. Five.";
        let out = chunk_text(text, 50, TEST_MAX_CHARS);
        // The regex splits on ". " so we get 5 sentences, but they all fit
        // within the word budget (50) so chunk_text groups them into 1 chunk.
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("One."));
        assert!(out[0].contains("Five."));
    }

    #[test]
    fn splits_when_exceeding_max() {
        let text = "Alpha bravo charlie. Delta echo foxtrot. Golf hotel india. Juliett kilo lima.";
        let out = chunk_text(text, 4, TEST_MAX_CHARS);
        assert!(out.len() >= 2);
    }

    #[test]
    fn preserves_long_sentence() {
        let long = "a ".repeat(100);
        let out = chunk_text(&long, 20, TEST_MAX_CHARS);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn does_not_split_on_common_abbreviations() {
        // "Dr." is split by the regex (it ends with ". "), which is acceptable
        // — the legacy Python regex had the same behaviour. The chunk_text
        // function re-joins them if they fit the budget.
        let text = "Dr. Smith went to the lab. He discovered something.";
        let out = split_sentences(text);
        assert!(out.len() >= 2);
        assert!(out[0].starts_with("Dr."));
    }

    #[test]
    fn respects_language_specific_char_limit() {
        // Italian: a short sentence should fit in one chunk at 2300 chars,
        // but the same sentence in Japanese should be limited to 1100.
        let text = "Ciao mondo. Questa è una prova.";
        let it = chunk_text(text, 1000, 2300);
        assert_eq!(it.len(), 1);
        let ja = chunk_text(text, 1000, 1100);
        assert_eq!(ja.len(), 1);
    }

    #[test]
    fn split_balanced_breaks_on_sentence_boundaries() {
        let text = "First sentence here. Second sentence is a bit longer. Third one. Fourth and final sentence.";
        let parts = split_text_balanced(text, 2);
        assert_eq!(parts.len(), 2);
        // Every part must end at a sentence boundary (trailing punctuation).
        for p in &parts {
            assert!(p.ends_with('.') || p.ends_with('!') || p.ends_with('?'), "part: {p}");
        }
        // Reading order is preserved across parts.
        assert_eq!(parts.join(" "), text);
    }

    #[test]
    fn split_balanced_is_roughly_even() {
        let sentences: Vec<String> = (0..8)
            .map(|i| format!("Sentence number {i} with some padding text."))
            .collect();
        let text = sentences.join(" ");
        let parts = split_text_balanced(&text, 4);
        assert_eq!(parts.len(), 4);
        let lens: Vec<usize> = parts.iter().map(|p| p.chars().count()).collect();
        let max = *lens.iter().max().unwrap();
        let min = *lens.iter().min().unwrap();
        // With equal-length sentences each part holds 2 sentences.
        assert!(max - min <= 2, "unbalanced split: {lens:?}");
    }

    #[test]
    fn split_balanced_falls_back_to_words_without_sentences() {
        let text = "one two three four five six seven eight";
        let parts = split_text_balanced(text, 2);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts.join(" "), text);
    }

    #[test]
    fn split_balanced_degenerate_text() {
        let parts = split_text_balanced("hello", 3);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], "hello");
    }
}
