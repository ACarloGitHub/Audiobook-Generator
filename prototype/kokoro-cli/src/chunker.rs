use anyhow::Result;

/// Split a chapter into chunks suitable for TTS.
///
/// The strategy is intentionally simple: split on sentence-terminating
/// punctuation (`.`, `!`, `?`, newlines), then greedily group sentences
/// into chunks of at most `max_words` words. We split on sentence
/// boundaries so the model never sees an unfinished thought at the
/// end of a chunk.
pub fn chunk_text(text: &str, max_words: usize) -> Vec<String> {
    let sentences = split_sentences(text);
    let mut chunks: Vec<String> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_words: usize = 0;

    for sentence in sentences {
        let trimmed = sentence.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let words = trimmed.split_whitespace().count();

        if current_words + words > max_words && !current.is_empty() {
            chunks.push(current.join(" "));
            current.clear();
            current_words = 0;
        }
        current.push(trimmed);
        current_words += words;
    }

    if !current.is_empty() {
        chunks.push(current.join(" "));
    }

    if chunks.is_empty() {
        // The whole text is empty or unchunkable. Return the original
        // trimmed text so the caller still gets a single chunk to process.
        chunks.push(text.trim().to_string());
    }

    chunks
}

/// Cheap regex-based sentence splitter. Good enough for the prototype;
/// the real core will swap in `sentencex` with the right API once we
/// pin a stable version.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?' | '\n') {
            // Look ahead: if the next non-space char is a letter, this
            // period is mid-word (e.g. "Dr. Smith") and we skip it.
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
        // Long sentence is kept as one chunk even if it exceeds max_words.
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
