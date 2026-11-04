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
    let mut final_chunks: Vec<String> = Vec::new();

    // Never merge two paragraphs into one chunk: the model reads each chunk
    // as a self-contained utterance, and a paragraph break is where a
    // narrator would pause. `split_paragraphs` also re-joins hard-wrapped
    // lines (TXT files) that do not end a sentence.
    for paragraph in split_paragraphs(text) {
        let sentences = split_sentences(&paragraph);
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

            // A single sentence that cannot fit the budget even alone: split
            // it at the most natural boundary instead of letting the chunk
            // overflow the engine limit.
            if current.is_empty() && chars > effective_max_chars {
                final_chunks.extend(hard_split(&trimmed, effective_max_chars));
                continue;
            }

            let would_exceed_words = current_words + words > max_words && !current.is_empty();
            let would_exceed_chars =
                current_chars + chars + 1 > effective_max_chars && !current.is_empty();

            if would_exceed_words || would_exceed_chars {
                final_chunks.push(current.join(" "));
                current.clear();
                current_words = 0;
                current_chars = 0;
            }
            current.push(trimmed);
            current_words += words;
            current_chars += chars + 1;
        }

        if !current.is_empty() {
            final_chunks.push(current.join(" "));
        }
    }

    if final_chunks.is_empty() {
        final_chunks.push(text.trim().to_string());
    }

    final_chunks
}

/// Characters that may close a sentence (quotation marks, brackets) before
/// the whitespace that starts the next one.
const SENTENCE_CLOSERS: [char; 9] = ['»', '"', '\'', ')', ']', '”', '’', '›', '’'];

/// Punctuation that can end a sentence.
const SENTENCE_END: [char; 4] = ['.', '!', '?', '…'];

/// Italian abbreviations that end with a period without ending the sentence.
/// Compared lowercased, without the trailing dot.
const ABBREVIATIONS: &[&str] = &[
    "sig", "sig.ra", "sig.na", "dott", "dott.ssa", "prof", "prof.ssa", "avv", "ing",
    "geom", "arch", "on", "sen", "art", "pag", "pagg", "fig", "tab", "cap", "ecc",
    "es", "cfr", "n", "nn", "p", "pp", "s", "ss", "mons", "rev", "gen", "col",
    "ten", "amm", "dir", "sez", "tel", "fax", "ca", "a.c", "p.c", "s.p.a",
    "s.r.l", "s.n.c", "s.a.s", "etc", "vs", "mr", "mrs", "ms", "dr", "st", "jr",
    "sr", "vol", "ed", "tav", "cit", "ibid",
];

/// Split the text into paragraphs. A newline ends a paragraph when the line
/// ends with sentence punctuation, a closing quote or a colon; otherwise the
/// line is treated as hard-wrapped prose and re-joined with the next one.
/// Blank lines always break.
fn split_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.trim().is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
        if ends_sentence(trimmed) {
            paragraphs.push(std::mem::take(&mut current));
        }
    }
    if !current.trim().is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

fn ends_sentence(line: &str) -> bool {
    line.trim_end()
        .chars()
        .last()
        .map(|c| matches!(c, '.' | '!' | '?' | '…' | ':' | ';' | '»' | '"' | '”' | '’' | ')' | ']'))
        .unwrap_or(false)
}

/// True when the period at `dot` closes an abbreviation or an initial rather
/// than a sentence ("Dott.", "Sig.ra", "J. R. R.", "S. Maria").
fn closes_abbreviation(chars: &[char], dot: usize) -> bool {
    let mut start = dot;
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '.' || c == '\'' {
            start -= 1;
        } else {
            break;
        }
    }
    let token: String = chars[start..dot].iter().collect::<String>().to_lowercase();
    let token = token.trim_matches('.').to_string();
    if token.is_empty() {
        return false;
    }
    // Single initial: "J.", "R.", "S.".
    if token.chars().count() == 1 && token.chars().all(|c| c.is_alphabetic()) {
        return true;
    }
    if ABBREVIATIONS.contains(&token.as_str()) {
        return true;
    }
    // Already-dotted abbreviations keep their internal dots: "s.r.l.".
    let collapsed: String = token.chars().filter(|c| *c != '.').collect();
    ABBREVIATIONS.contains(&collapsed.as_str())
}

/// Words that open a new clause in Italian narrative prose. Splitting before
/// them keeps the break on a natural pause instead of an arbitrary word.
const CONJUNCTIONS: [&str; 14] = [
    "e", "o", "ma", "però", "pero", "quindi", "dunque", "allora", "cioè", "cioe", "infatti",
    "mentre", "poiché", "poiche",
];

/// Break an over-long fragment at the most natural boundary available.
///
/// Mirrors the legacy `split_long_sentence` cascade: first after a comma,
/// semicolon or colon, then before a conjunction, and only as a last resort
/// between words. Every returned part is at most `max_chars` characters.
fn hard_split(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    for phrase in pack_units(&split_after_punctuation(text), max_chars) {
        if phrase.chars().count() <= max_chars {
            out.push(phrase);
            continue;
        }
        for clause in pack_units(&split_before_conjunction(&phrase), max_chars) {
            if clause.chars().count() <= max_chars {
                out.push(clause);
            } else {
                out.extend(split_words(&clause, max_chars));
            }
        }
    }
    out
}

/// Split after `,` `;` `:`, keeping the punctuation attached to the piece it
/// closes. A comma between digits is a decimal separator ("3,14") and is
/// never a break point.
fn split_after_punctuation(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, c) in chars.iter().enumerate() {
        current.push(*c);
        let decimal_comma = *c == ','
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).map(|n| n.is_ascii_digit()).unwrap_or(false);
        if matches!(*c, ',' | ';' | ':') && !decimal_comma {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    if out.is_empty() {
        out.push(text.trim().to_string());
    }
    out
}

/// Split before a conjunction, so the conjunction opens the next fragment.
fn split_before_conjunction(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for word in text.split_whitespace() {
        let bare = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        let starts_clause = CONJUNCTIONS.contains(&bare.as_str());
        if starts_clause && !current.is_empty() {
            out.push(current.join(" "));
            current.clear();
        }
        current.push(word);
    }
    if !current.is_empty() {
        out.push(current.join(" "));
    }
    if out.is_empty() {
        out.push(text.trim().to_string());
    }
    out
}

/// Greedily join units with a single space while the budget allows.
fn pack_units(units: &[String], max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for unit in units {
        let unit_len = unit.chars().count();
        if !current.is_empty() && current.chars().count() + unit_len + 1 > max_chars {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(unit);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Last-resort split between words.
fn split_words(text: &str, max_chars: usize) -> Vec<String> {
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
/// Splits on sentence-ending punctuation (`.`, `!`, `?`, `…`), keeping the
/// punctuation attached to the preceding sentence.
///
/// Guards that keep a chunk boundary away from a mid-sentence point:
/// - decimal points (`3.14`) are not sentence ends;
/// - Italian abbreviations and initials (`Dott.`, `Sig.ra`, `J. R. R.`) are
///   not sentence ends;
/// - a period followed by a lowercase word is treated as a continuation,
///   which catches abbreviations we do not list;
/// - ellipses (`...`, `…`) do end a sentence, after the whole run.
pub fn split_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0usize;

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if !SENTENCE_END.contains(&c) {
            i += 1;
            continue;
        }

        // Consume a run of repeated punctuation: "...", "!!", "?!", "……".
        let mut end = i;
        while end + 1 < chars.len() && SENTENCE_END.contains(&chars[end + 1]) {
            end += 1;
        }

        // A period between digits is a decimal separator, not a full stop.
        if c == '.' {
            let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_digit = chars.get(end + 1).map(|n| n.is_ascii_digit()).unwrap_or(false);
            if prev_digit && next_digit {
                i = end + 1;
                continue;
            }
        }

        // Abbreviations and initials only apply to a single period, not to a
        // run like "...".
        if c == '.' && end == i && closes_abbreviation(&chars, i) {
            i += 1;
            continue;
        }

        // A sentence may close with a quotation mark or a bracket right after
        // the punctuation («Non lo so.» / "Basta." / (davvero.)).
        while end + 1 < chars.len() && SENTENCE_CLOSERS.contains(&chars[end + 1]) {
            end += 1;
        }
        let next_is_ws = end + 1 >= chars.len() || chars[end + 1].is_whitespace();
        if !next_is_ws {
            i = end + 1;
            continue;
        }

        // "Basta. poi" is far more often an abbreviation or a numbering
        // artifact than a real sentence break: keep it together.
        if c == '.' {
            let next_word = chars[end + 1..].iter().find(|ch| !ch.is_whitespace());
            if let Some(nc) = next_word {
                if nc.is_lowercase() {
                    i = end + 1;
                    continue;
                }
            }
        }

        let sentence: String = chars[start..=end].iter().collect();
        let trimmed = sentence.trim().to_string();
        if !trimmed.is_empty() {
            out.push(trimmed);
        }
        let mut j = end + 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        start = j;
        i = j;
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

    #[test]
    fn hard_split_breaks_after_commas_first() {
        let text = "aaa bbb ccc, ddd eee fff, ggg hhh iii, lll mmm nnn";
        let out = hard_split(text, 20);
        assert_eq!(out.len(), 4, "parts: {out:?}");
        for part in out.iter().take(3) {
            assert!(part.ends_with(','), "expected a comma break, got: {part}");
        }
        assert_eq!(out.join(" "), text);
    }

    #[test]
    fn hard_split_falls_back_to_conjunctions() {
        let text = "alpha beta gamma e delta epsilon zeta e eta theta iota";
        let out = hard_split(text, 20);
        assert!(out.len() >= 2, "parts: {out:?}");
        for part in out.iter().skip(1) {
            assert!(part.starts_with("e "), "expected a conjunction break, got: {part}");
        }
        for part in &out {
            assert!(part.chars().count() <= 20, "over budget: {part}");
        }
        assert_eq!(out.join(" "), text);
    }

    #[test]
    fn hard_split_uses_words_as_last_resort() {
        let text = "one two three four five six";
        let out = hard_split(text, 10);
        assert_eq!(out, vec!["one two", "three four", "five six"]);
        for part in &out {
            assert!(part.chars().count() <= 10);
        }
    }

    #[test]
    fn sentences_split_after_guillemets() {
        let text = "«Non lo so.» Disse che sarebbe tornato.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert_eq!(out[0], "«Non lo so.»");
        assert_eq!(out[1], "Disse che sarebbe tornato.");
    }

    #[test]
    fn sentences_split_after_closing_quote() {
        let text = "\"Basta.\" Poi uscì senza dire altro.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert_eq!(out[0], "\"Basta.\"");
    }

    #[test]
    fn sentences_do_not_split_inside_numbers() {
        let text = "Il numero 3.14 è famoso.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 1, "parts: {out:?}");
    }

    #[test]
    fn chunk_text_ends_every_chunk_at_a_sentence_boundary() {
        let text = "Prima frase abbastanza lunga per contare. Seconda frase altrettanto lunga qui. Terza frase finale.";
        let out = chunk_text(text, 1000, 60);
        assert!(out.len() >= 2, "chunks: {out:?}");
        for chunk in &out {
            assert!(
                chunk.ends_with('.') || chunk.ends_with('!') || chunk.ends_with('?'),
                "chunk does not end at a sentence boundary: {chunk}"
            );
        }
        assert_eq!(out.join(" "), text);
    }

    #[test]
    fn italian_abbreviations_do_not_end_a_sentence() {
        let text = "Il Dott. Rossi arrivò tardi. Poi uscì.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert_eq!(out[0], "Il Dott. Rossi arrivò tardi.");
    }

    #[test]
    fn initials_do_not_end_a_sentence() {
        let text = "J. R. R. Tolkien scrisse il libro. Fu un successo.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert!(out[0].starts_with("J. R. R. Tolkien"));
    }

    #[test]
    fn ascii_ellipsis_ends_a_sentence() {
        let text = "Non lo so... Disse altro.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert_eq!(out[0], "Non lo so...");
    }

    #[test]
    fn unicode_ellipsis_ends_a_sentence() {
        let text = "Non lo so… Disse altro.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 2, "parts: {out:?}");
        assert_eq!(out[0], "Non lo so…");
    }

    #[test]
    fn period_followed_by_lowercase_is_a_continuation() {
        let text = "Il numero è 3. poi segue.";
        let out = split_sentences(text);
        assert_eq!(out.len(), 1, "parts: {out:?}");
    }

    #[test]
    fn decimal_comma_is_never_a_break_point() {
        let text = "Il prezzo è 3,14 euro e sembra giusto.";
        let out = hard_split(text, 18);
        assert!(
            out.iter().any(|p| p.contains("3,14")),
            "decimal comma was split: {out:?}"
        );
    }

    #[test]
    fn chunks_never_cross_a_paragraph_break() {
        let text = "Prima frase del paragrafo.\nSeconda frase del paragrafo.";
        let out = chunk_text(text, 1000, 1000);
        assert_eq!(out.len(), 2, "chunks: {out:?}");
        assert_eq!(out[0], "Prima frase del paragrafo.");
        assert_eq!(out[1], "Seconda frase del paragrafo.");
    }

    #[test]
    fn long_paragraph_is_split_without_crossing_to_the_next_one() {
        let long = "parola ".repeat(40) + "fine.";
        let text = format!("{long}\nAltro paragrafo.");
        let out = chunk_text(&text, 1000, 100);
        assert!(out.len() >= 2, "chunks: {out:?}");
        assert_eq!(out.last().unwrap(), "Altro paragrafo.");
        for chunk in &out[..out.len() - 1] {
            assert!(!chunk.contains("Altro paragrafo"), "crossed paragraph: {chunk}");
        }
    }

    #[test]
    fn hard_wrapped_lines_are_rejoined() {
        let text = "una riga\nspezzata a metà\n\nnuovo paragrafo.";
        let out = chunk_text(text, 1000, 1000);
        assert_eq!(out.len(), 2, "chunks: {out:?}");
        assert_eq!(out[0], "una riga spezzata a metà");
        assert_eq!(out[1], "nuovo paragrafo.");
    }
}
