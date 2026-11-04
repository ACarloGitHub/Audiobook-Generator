use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use tracing::{info, warn};

/// One chapter of an EPUB, already stripped of HTML.
pub struct Chapter {
    pub title: String,
    pub text: String,
}

/// Book-level container. The title defaults
/// to the EPUB file stem if the OPF metadata has no `dc:title`.
pub struct Book {
    pub title: String,
    /// English label of the document language (e.g. "Italian"), resolved
    /// from `dc:language` / `<html lang>` when available.
    pub language: Option<String>,
    pub chapters: Vec<Chapter>,
}

/// Open an EPUB, read its Table of Contents (toc.ncx or EPUB 3 NAV), and
/// return one entry per chapter listed there.
///
/// Faithful port of the legacy Python `extract_chapters_from_epub` logic:
/// 1. Try the ToC first. The ToC is what the publisher used to mark
///    chapters, so it is the most reliable way to skip cover pages,
///    copyright notices, indexes, and other front/back matter.
/// 2. Filter out ToC entries whose extracted text is under 50 chars
///    (cover images, blank pages, etc.). This matches the legacy
///    behaviour.
/// 3. If the ToC is missing or empty, fall back to walking the spine
///    and treating the whole book as a single "Chapter_01_Full_Book".
/// 4. If even the fallback produces nothing, error out.
pub fn extract_chapters(path: &Path) -> Result<Vec<Chapter>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid ZIP / EPUB", path.display()))?;

    let (opf_path, opf_bytes) = read_container_xml(&mut zip)?;
    let opf = String::from_utf8(opf_bytes).context("OPF is not UTF-8")?;
    let manifest = parse_opf_manifest(&opf)?;
    let spine = parse_opf_spine(&opf, &manifest)?;

    let mut chapters: Vec<Chapter> = Vec::new();
    let mut processed_hrefs: HashSet<String> = HashSet::new();
    let mut chapter_count: usize = 0;

    let toc_entries = try_toc_ncx(&mut zip, &opf_path)
        .or_else(|| try_nav_document(&mut zip, &opf_path, &manifest));

    if let Some(entries) = toc_entries {
        for (raw_title, href) in entries {
            let href_base = href.split('#').next().unwrap_or(&href).to_string();
            if processed_hrefs.contains(&href_base) {
                continue;
            }
            let inner = resolve_path(&opf_path, &href_base);
            let Ok(bytes) = read_zip_entry(&mut zip, &inner) else {
                warn!("ToC references missing entry {inner}");
                continue;
            };
            let Ok(xhtml) = String::from_utf8(bytes) else {
                continue;
            };
            let text = html_to_text(&xhtml);
            if text.trim().len() < 50 {
                continue;
            }
            chapter_count += 1;
            let clean_title = sanitize_title(&raw_title);
            let key = format!("Chapter_{:02}_{}", chapter_count, clean_title);
            let key = if key.len() > 80 {
                format!("{}...", &key[..77])
            } else {
                key
            };
            processed_hrefs.insert(href_base);
            chapters.push(Chapter { title: key, text });
        }
        if !chapters.is_empty() {
            info!("successfully extracted {} chapter(s) via ToC", chapters.len());
            return Ok(chapters);
        }
    }

    warn!("ToC not found or did not yield chapters. Using fallback sequential extraction.");
    let mut full_text = String::new();
    for (_id, href) in &spine {
        let inner = resolve_path(&opf_path, href);
        let Ok(bytes) = read_zip_entry(&mut zip, &inner) else {
            continue;
        };
        let Ok(xhtml) = String::from_utf8(bytes) else {
            continue;
        };
        let text = html_to_text(&xhtml);
        if !text.trim().is_empty() {
            if !full_text.is_empty() {
                full_text.push_str("\n\n");
            }
            full_text.push_str(text.trim());
        }
    }

    if full_text.is_empty() {
        bail!("EPUB contained no readable text content");
    }
    info!("fallback: treating entire book content as a single chapter");
    Ok(vec![Chapter {
        title: "Chapter_01_Full_Book".to_string(),
        text: full_text,
    }])
}

fn sanitize_title(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .replace(' ', "_")
}

/// Document language of the book, as the English label the TTS engines use
/// (e.g. "Italian"). Read from the OPF `dc:language` (EPUB 2 and 3) and
/// resolved through [`language_label`]. Returns `None` for non-EPUB inputs
/// or when the metadata is missing/unknown.
pub fn read_language(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext != "epub" {
        return None;
    }
    let file = File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let (_opf_path, opf_bytes) = read_container_xml(&mut zip).ok()?;
    let opf = String::from_utf8(opf_bytes).ok()?;
    let patterns = [
        r"(?is)<dc:language[^>]*>\s*([^<]+?)\s*</dc:language>",
        r#"(?is)\bxml:lang=["']([^"']+)["']"#,
        r#"(?is)<html[^>]*\blang=["']([^"']+)["']"#,
    ];
    for pattern in patterns {
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        if let Some(caps) = re.captures(&opf) {
            if let Some(code) = caps.get(1) {
                if let Some(label) = language_label(code.as_str()) {
                    return Some(label);
                }
            }
        }
    }
    None
}

/// Map an ISO-639 code ("it", "it-IT", "en_US") to the English label used
/// in `engine_registry.json`. Unknown languages return `None` so callers
/// can fall back to "Auto" instead of sending a label the engine rejects.
pub fn language_label(raw: &str) -> Option<String> {
    let code = raw
        .trim()
        .split(['-', '_'])
        .next()
        .unwrap_or(raw)
        .to_ascii_lowercase();
    let label = match code.as_str() {
        "it" => "Italian",
        "en" => "English",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" => "Chinese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "hu" => "Hungarian",
        "lv" => "Latvian",
        "ka" => "Georgian",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "tr" => "Turkish",
        "sv" => "Swedish",
        "no" | "nb" | "nn" => "Norwegian",
        "da" => "Danish",
        "fi" => "Finnish",
        "el" => "Greek",
        "he" => "Hebrew",
        "th" => "Thai",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "ms" => "Malay",
        "my" => "Burmese",
        "km" => "Khmer",
        "lo" => "Lao",
        "sw" => "Swahili",
        "tl" => "Filipino",
        "yue" => "Cantonese",
        _ => return None,
    };
    Some(label.to_string())
}

fn read_container_xml<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<(String, Vec<u8>)> {
    let mut container = String::new();
    zip.by_name("META-INF/container.xml")
        .context("missing META-INF/container.xml — not a valid EPUB")?
        .read_to_string(&mut container)
        .context("reading container.xml")?;

    let mut reader = Reader::from_str(&container);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if matches_rootfile(&e) => {
                for attr in e.attributes() {
                    let attr = attr.context("invalid attribute")?;
                    if attr.key.as_ref() == b"full-path" {
                        let path = attr
                            .unescape_value()
                            .context("non-UTF-8 full-path")?
                            .into_owned();
                        let bytes = read_zip_entry(zip, &path)
                            .with_context(|| format!("reading OPF at {path}"))?;
                        return Ok((path, bytes));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("container.xml: {e}")),
            _ => {}
        }
        buf.clear();
    }
    bail!("META-INF/container.xml did not reference an OPF rootfile")
}

fn matches_rootfile(e: &BytesStart) -> bool {
    e.name().as_ref() == b"rootfile"
}

fn parse_opf_manifest(opf: &str) -> Result<HashMap<String, ManifestItem>> {
    let mut out: HashMap<String, ManifestItem> = Default::default();
    let mut reader = Reader::from_str(opf);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_manifest = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"manifest" => in_manifest = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"manifest" => in_manifest = false,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if in_manifest => {
                if e.name().as_ref() == b"item" {
                    let mut id = None;
                    let mut href = None;
                    let mut properties = None;
                    for attr in e.attributes() {
                        let attr = attr?;
                        match attr.key.as_ref() {
                            b"id" => id = Some(attr.unescape_value()?.into_owned()),
                            b"href" => href = Some(attr.unescape_value()?.into_owned()),
                            b"properties" => properties = Some(attr.unescape_value()?.into_owned()),
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(href)) = (id, href) {
                        out.insert(
                            id,
                            ManifestItem {
                                href,
                                properties: properties.unwrap_or_default(),
                            },
                        );
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("OPF manifest: {e}"),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

struct ManifestItem {
    href: String,
    properties: String,
}

fn parse_opf_spine(
    opf: &str,
    manifest: &HashMap<String, ManifestItem>,
) -> Result<Vec<(String, String)>> {
    let mut spine: Vec<(String, String)> = Vec::new();
    let mut in_spine = false;
    let mut buf = Vec::new();
    let mut reader = Reader::from_str(opf);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"spine" => in_spine = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"spine" => in_spine = false,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if in_spine => {
                if e.name().as_ref() == b"itemref" {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"idref" {
                            let id = attr.unescape_value()?.into_owned();
                            if let Some(item) = manifest.get(&id) {
                                spine.push((id, item.href.clone()));
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("OPF spine: {e}"),
            _ => {}
        }
        buf.clear();
    }
    Ok(spine)
}

fn try_toc_ncx<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    opf_path: &str,
) -> Option<Vec<(String, String)>> {
    let opf_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let ncx_path = if opf_dir.is_empty() {
        "toc.ncx".to_string()
    } else {
        format!("{opf_dir}/toc.ncx")
    };
    let bytes = read_zip_entry(zip, &ncx_path).ok()?;
    let text = String::from_utf8(bytes).ok()?;

    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut in_nav_map = false;
    let mut current_title: Option<String> = None;
    let mut current_src: Option<String> = None;
    let mut in_label = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"navMap" => {
                in_nav_map = true;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"navMap" => {
                in_nav_map = false;
            }
            Ok(Event::Start(e)) if in_nav_map && e.name().as_ref() == b"navPoint" => {
                current_title = None;
                current_src = None;
            }
            Ok(Event::End(e)) if in_nav_map && e.name().as_ref() == b"navPoint" => {
                if let (Some(title), Some(src)) = (current_title.take(), current_src.take()) {
                    entries.push((title, src));
                }
            }
            Ok(Event::Start(e)) if in_nav_map && e.name().as_ref() == b"navLabel" => {
                in_label = true;
            }
            Ok(Event::End(e)) if in_nav_map && e.name().as_ref() == b"navLabel" => {
                in_label = false;
            }
            Ok(Event::Start(e)) if in_nav_map && e.name().as_ref() == b"content" => {
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        if attr.key.as_ref() == b"src" {
                            if let Ok(s) = attr.unescape_value() {
                                current_src = Some(s.into_owned());
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) if in_nav_map && e.name().as_ref() == b"content" => {
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        if attr.key.as_ref() == b"src" {
                            if let Ok(s) = attr.unescape_value() {
                                current_src = Some(s.into_owned());
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let s = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                if in_nav_map && in_label {
                    let merged = match current_title.as_mut() {
                        Some(existing) => {
                            existing.push_str(&s);
                            None
                        }
                        None => Some(s),
                    };
                    if merged.is_some() {
                        current_title = merged;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

fn try_nav_document<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    opf_path: &str,
    manifest: &HashMap<String, ManifestItem>,
) -> Option<Vec<(String, String)>> {
    let nav_item = manifest
        .values()
        .find(|m| m.properties.split_whitespace().any(|p| p == "nav"))?;
    let nav_path = resolve_path(opf_path, &nav_item.href);
    let bytes = read_zip_entry(zip, &nav_path).ok()?;
    let text = String::from_utf8(bytes).ok()?;

    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_nav = false;
    let mut in_ol = false;
    let mut ol_depth: usize = 0;
    let mut current_title: Option<String> = None;
    let mut current_href: Option<String> = None;
    let mut entries: Vec<(String, String)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"nav" => {
                in_nav = true;
            }
            Ok(Event::Start(e)) if in_nav && e.name().as_ref() == b"ol" => {
                in_ol = true;
                ol_depth += 1;
            }
            Ok(Event::End(e)) if in_nav && e.name().as_ref() == b"ol" => {
                ol_depth -= 1;
                if ol_depth == 0 {
                    in_ol = false;
                }
            }
            Ok(Event::Start(e)) if in_nav && in_ol && ol_depth == 1 && e.name().as_ref() == b"li" => {
                current_title = None;
                current_href = None;
            }
            Ok(Event::End(e)) if in_nav && in_ol && ol_depth == 1 && e.name().as_ref() == b"li" => {
                if let (Some(title), Some(href)) = (current_title.take(), current_href.take()) {
                    entries.push((title, href));
                }
            }
            Ok(Event::Start(e)) if in_nav && in_ol && ol_depth == 1 && e.name().as_ref() == b"a" => {
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        if attr.key.as_ref() == b"href" {
                            if let Ok(s) = attr.unescape_value() {
                                current_href = Some(s.into_owned());
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(t)) if in_nav && in_ol && ol_depth == 1 => {
                let s = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                if !s.trim().is_empty() {
                    current_title = Some(s);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

fn read_zip_entry<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let mut f = zip
        .by_name(name)
        .with_context(|| format!("entry not found: {name}"))?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf).context("reading zip entry")?;
    Ok(buf)
}

fn resolve_path(opf_path: &str, href: &str) -> String {
    let opf_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    if href.starts_with('/') {
        href.trim_start_matches('/').to_string()
    } else if opf_dir.is_empty() {
        href.to_string()
    } else {
        format!("{opf_dir}/{href}")
    }
}

fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut reader = Reader::from_str(html);
    // Keep whitespace: the spaces between inline tags (`<span>di </span>`)
    // carry word boundaries, and paragraph breaks are rebuilt from block
    // tags below. Trimming here fused words and paragraphs together.
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let skip_tags: &[&[u8]] = &[
        b"script", b"style", b"nav", b"header", b"footer", b"aside", b"figure",
        b"img", b"a", b"sup", b"sub", b"map", b"area", b"noscript",
        b"iframe", b"object", b"embed", b"video", b"audio", b"source", b"track",
    ];
    let mut skip_depth: usize = 0;
    let break_tags: &[&[u8]] = &[b"br", b"hr"];
    let block_tags: &[&[u8]] = &[
        b"p", b"div", b"h1", b"h2", b"h3", b"h4", b"h5", b"h6",
        b"li", b"ul", b"ol", b"blockquote", b"section", b"article",
        b"tr", b"td", b"th", b"table", b"pre", b"dd", b"dt", b"figcaption",
    ];

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if skip_tags.iter().any(|t| *t == name.as_slice()) {
                    skip_depth += 1;
                } else if skip_depth == 0 {
                    if break_tags.iter().any(|t| *t == name.as_slice())
                        || block_tags.iter().any(|t| *t == name.as_slice())
                    {
                        out.push('\n');
                    }
                }
            }
            Ok(Event::End(e)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                } else {
                    let name = e.name().as_ref().to_vec();
                    if block_tags.iter().any(|t| *t == name.as_slice()) {
                        out.push('\n');
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                if skip_tags.iter().any(|t| *t == name.as_slice()) {
                    // Self-closing skip tag: stays out.
                } else if break_tags.iter().any(|t| *t == name.as_slice()) {
                    out.push('\n');
                }
            }
            Ok(Event::Text(t)) => {
                if skip_depth == 0 {
                    let unescaped = t.unescape().unwrap_or_default();
                    push_collapsed_text(&mut out, &unescaped);
                }
            }
            Ok(Event::CData(c)) => {
                if skip_depth == 0 {
                    push_collapsed_text(&mut out, std::str::from_utf8(c.as_ref()).unwrap_or(""));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("XHTML parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    normalize_text(&out)
}

/// Append one text node, collapsing every whitespace run (including the
/// newlines used to indent the XHTML source) to a single space. Paragraph
/// breaks are added separately from block tags, so the indentation of the
/// source must not leak into the paragraph structure.
fn push_collapsed_text(out: &mut String, text: &str) {
    let mut pending_space = false;
    for c in text.chars() {
        match c {
            // Soft hyphen and zero-width characters only confuse the model.
            '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => continue,
            c if c.is_whitespace() => pending_space = true,
            c => {
                if pending_space && !out.is_empty() && !out.ends_with([' ', '\n']) {
                    out.push(' ');
                }
                pending_space = false;
                out.push(c);
            }
        }
    }
    if pending_space && !out.is_empty() && !out.ends_with([' ', '\n']) {
        out.push(' ');
    }
}

/// Final normalisation: collapse spaces, turn runs of newlines into a single
/// paragraph break, and trim the ends.
fn normalize_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut pending_space = false;
    let mut pending_newline = false;
    for c in raw.chars() {
        if c == '\n' {
            pending_newline = true;
            pending_space = false;
        } else if c.is_whitespace() {
            if !pending_newline {
                pending_space = true;
            }
        } else {
            if pending_newline {
                if !out.is_empty() {
                    out.push('\n');
                }
                pending_newline = false;
            } else if pending_space && !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_spaces_between_inline_tags() {
        let html = "<p><span>con l'umore di </span><span>Elia</span><span>. Era seduto</span></p>";
        assert_eq!(html_to_text(html), "con l'umore di Elia. Era seduto");
    }

    #[test]
    fn keeps_whitespace_only_spans_as_word_separator() {
        let html = "<p><span>14 marzo 1952.</span><span> </span><span>La citta</span></p>";
        assert_eq!(html_to_text(html), "14 marzo 1952. La citta");
    }

    #[test]
    fn paragraphs_become_newlines_not_fused_sentences() {
        let html = "<h2><span>Titolo</span></h2><p><span>Prima.</span></p><p><span>Seconda.</span></p>";
        assert_eq!(html_to_text(html), "Titolo\nPrima.\nSeconda.");
    }

    #[test]
    fn source_indentation_does_not_create_paragraphs() {
        let html = "<p>\n  <span>Una</span>\n  <span>frase.</span>\n</p>";
        assert_eq!(html_to_text(html), "Una frase.");
    }

    #[test]
    fn skips_scripts_and_links_but_keeps_surrounding_text() {
        let html = "<p>Prima <a href=\"#n\">nota</a> dopo.<script>x</script></p>";
        assert_eq!(html_to_text(html), "Prima dopo.");
    }

    #[test]
    fn removes_soft_hyphens() {
        assert_eq!(html_to_text("<p>im\u{00AD}portante</p>"), "importante");
    }

    #[test]
    fn maps_iso_codes_to_engine_labels() {
        assert_eq!(language_label("it").as_deref(), Some("Italian"));
        assert_eq!(language_label("en-US").as_deref(), Some("English"));
        assert_eq!(language_label("pt_BR").as_deref(), Some("Portuguese"));
        assert_eq!(language_label("xx"), None);
    }

    #[test]
    fn real_epub_has_no_fused_words() {
        let path = Path::new("../test_files/test_ebook_it.epub");
        if !path.exists() {
            return;
        }
        let chapters = extract_chapters(path).expect("extract test epub");
        assert_eq!(read_language(path).as_deref(), Some("Italian"));
        let text = &chapters[0].text;
        assert!(!text.contains("diElia"), "fused word still present: {text:.200}");
        assert!(text.contains("di Elia"), "expected 'di Elia' in: {text:.200}");
        assert!(text.contains('\n'), "expected paragraph breaks in chapter text");
    }
}
