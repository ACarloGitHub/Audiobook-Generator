//! Input format loaders: turn any supported document into the same
//! `Book`/`Chapter` structure the EPUB parser produces, so the whole
//! synthesis pipeline downstream stays unchanged.
//!
//! Supported: EPUB (delegated to `epub.rs`), TXT, Markdown, DOCX, JSON.
//! Deliberately NOT supported: legacy binary .doc (opaque format) and
//! PDF (text extraction is unreliable — deferred).

use std::path::Path;

use anyhow::{bail, Context, Result};

pub use crate::epub::{Book, Chapter};

/// Parse any supported document into a `Book` (title + chapters).
pub fn parse_document(path: &Path) -> Result<Book> {
    let chapters = extract_chapters_from(path)?;
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    Ok(Book { title, chapters })
}

/// Extract chapters from any supported document, dispatching on the
/// file extension.
pub fn extract_chapters_from(path: &Path) -> Result<Vec<Chapter>> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let chapters = match ext.as_str() {
        "epub" => crate::epub::extract_chapters(path)?,
        "txt" => vec![Chapter {
            title: "Full text".to_string(),
            text: read_text(path)?,
        }],
        "md" | "markdown" => parse_markdown(&read_text(path)?),
        "docx" => parse_docx(path)?,
        "json" => parse_json(&read_text(path)?)?,
        other => bail!(
            "unsupported input format '.{}'. Supported: epub, txt, md, docx, json",
            other
        ),
    };
    let chapters: Vec<Chapter> = chapters
        .into_iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if chapters.is_empty() {
        bail!("no readable text found in {}", path.display());
    }
    Ok(chapters)
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

// ---------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------

/// Split a Markdown document on level 1-2 headings; each section becomes
/// a chapter. Without headings the whole document is a single chapter.
/// Common markup (emphasis, links, images, code fences) is stripped.
fn parse_markdown(text: &str) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut current_title = String::new();
    let mut current = String::new();
    let mut in_code_fence = false;

    let mut push = |title: &str, body: &mut String| {
        let body_text = strip_markdown(body);
        if !body_text.trim().is_empty() {
            chapters.push(Chapter {
                title: if title.is_empty() {
                    "Section".to_string()
                } else {
                    title.to_string()
                },
                text: body_text,
            });
        }
        body.clear();
    };

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        let heading = line
            .trim_start()
            .strip_prefix("# ")
            .or_else(|| line.trim_start().strip_prefix("## "));
        if let Some(h) = heading {
            push(&current_title, &mut current);
            current_title = h.trim().to_string();
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    push(&current_title, &mut current);
    chapters
}

/// Remove common Markdown markers, keeping the readable text.
fn strip_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Images: drop entirely. Links: keep the visible text.
            '!' if chars.peek() == Some(&'[') => {
                let mut depth = 0;
                for c2 in chars.by_ref() {
                    if c2 == '[' {
                        depth += 1;
                    }
                    if c2 == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                if chars.peek() == Some(&'(') {
                    for c2 in chars.by_ref() {
                        if c2 == ')' {
                            break;
                        }
                    }
                }
            }
            '[' => {
                // Collect link text until ']', skip the (url) part.
                let mut label = String::new();
                for c2 in chars.by_ref() {
                    if c2 == ']' {
                        break;
                    }
                    label.push(c2);
                }
                if chars.peek() == Some(&'(') {
                    for c2 in chars.by_ref() {
                        if c2 == ')' {
                            break;
                        }
                    }
                }
                out.push_str(&label);
            }
            '*' | '_' | '`' | '#' => {}
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------

/// Extract paragraphs from word/document.xml. Paragraphs styled
/// "Heading1"/"Heading2" start a new chapter; everything else is body.
fn parse_docx(path: &Path) -> Result<Vec<Chapter>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid DOCX", path.display()))?;
    let mut xml = String::new();
    use std::io::Read as _;
    zip.by_name("word/document.xml")
        .context("word/document.xml not found (not a DOCX?)")?
        .read_to_string(&mut xml)?;

    let mut chapters: Vec<Chapter> = Vec::new();
    let mut current_title = String::new();
    let mut current = String::new();

    // Walk paragraphs manually: a paragraph starts at <w:p> or <w:p ...>
    // and ends at </w:p>. Within it, <w:pStyle w:val="..."/> gives the
    // style and <w:t>...</w:t> the text runs.
    for para in xml.split("</w:p>") {
        let style = extract_attr(para, "<w:pStyle", "w:val").unwrap_or_default();
        let mut text = String::new();
        let mut rest = para;
        while let Some(start) = rest.find("<w:t") {
            let after_tag = match rest[start..].find('>') {
                Some(gt) => &rest[start + gt + 1..],
                None => break,
            };
            let end = after_tag.find("</w:t>").unwrap_or(after_tag.len());
            text.push_str(&after_tag[..end]);
            rest = &after_tag[end..];
        }
        let text = xml_unescape(&text);
        if text.trim().is_empty() {
            continue;
        }
        let is_heading = style.starts_with("Heading1") || style.starts_with("Heading2")
            || style == "Title" || style == "Subtitle";
        if is_heading {
            if !current.trim().is_empty() {
                chapters.push(Chapter {
                    title: if current_title.is_empty() {
                        "Section".to_string()
                    } else {
                        current_title.clone()
                    },
                    text: current.trim().to_string(),
                });
            }
            current_title = text.trim().to_string();
            current.clear();
        } else {
            current.push_str(text.trim());
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        chapters.push(Chapter {
            title: if current_title.is_empty() {
                "Full text".to_string()
            } else {
                current_title
            },
            text: current.trim().to_string(),
        });
    }
    Ok(chapters)
}

fn extract_attr<'a>(xml: &'a str, tag: &str, attr: &str) -> Option<String> {
    let start = xml.find(tag)?;
    let end = xml[start..].find("/>").map(|e| start + e)?;
    let seg = &xml[start..end];
    let key = format!("{}=\"", attr);
    let vstart = seg.find(&key)? + key.len();
    let vend = seg[vstart..].find('"')? + vstart;
    Some(seg[vstart..vend].to_string())
}

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------

/// Accepted shapes:
/// - {"title": "...", "chapters": [{"title": "...", "text": "..."}, ...]}
/// - {"text": "..."}
/// - [{"title": "...", "text": "..."}, ...] or ["paragraph", ...]
fn parse_json(text: &str) -> Result<Vec<Chapter>> {
    let value: serde_json::Value =
        serde_json::from_str(text).context("invalid JSON input file")?;

    let chapter_from = |v: &serde_json::Value, idx: usize| -> Option<Chapter> {
        match v {
            serde_json::Value::String(s) => Some(Chapter {
                title: format!("Section {}", idx + 1),
                text: s.clone(),
            }),
            serde_json::Value::Object(_) => {
                let text = v.get("text").and_then(|t| t.as_str())?.to_string();
                let title = v
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Section")
                    .to_string();
                Some(Chapter { title, text })
            }
            _ => None,
        }
    };

    let chapters = match &value {
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .filter_map(|(i, v)| chapter_from(v, i))
            .collect(),
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Array(items)) = map.get("chapters") {
                items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| chapter_from(v, i))
                    .collect()
            } else if let Some(t) = map.get("text").and_then(|t| t.as_str()) {
                vec![Chapter {
                    title: "Full text".to_string(),
                    text: t.to_string(),
                }]
            } else {
                bail!("JSON input must have a 'chapters' array or a 'text' field");
            }
        }
        _ => bail!("JSON input must be an object or an array"),
    };
    Ok(chapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_splits_on_headings() {
        let md = "# Intro\nHello *world*.\n## Part two\nSee [this](http://x.y).\n";
        let chapters = parse_markdown(md);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "Intro");
        assert_eq!(chapters[0].text.trim(), "Hello world.");
        assert_eq!(chapters[1].title, "Part two");
        assert_eq!(chapters[1].text.trim(), "See this.");
    }

    #[test]
    fn markdown_without_headings_is_one_chapter() {
        let chapters = parse_markdown("Just some text.");
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Section");
    }

    #[test]
    fn json_accepts_chapters_shape() {
        let json = r#"{"title": "T", "chapters": [{"title": "A", "text": "hello"}, {"text": "world"}]}"#;
        let chapters = parse_json(json).unwrap();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "A");
        assert_eq!(chapters[1].text, "world");
    }

    #[test]
    fn json_accepts_plain_text() {
        let chapters = parse_json(r#"{"text": "only text"}"#).unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].text, "only text");
    }

    #[test]
    fn docx_splits_on_heading_styles() {
        // Build a minimal in-memory DOCX (a zip with word/document.xml).
        let document = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>First chapter</w:t></w:r></w:p>
<w:p><w:r><w:t>Body text one.</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Second chapter</w:t></w:r></w:p>
<w:p><w:r><w:t>Body text &amp; two.</w:t></w:r></w:p>
</w:body></w:document>"#;

        let tmp = std::env::temp_dir().join("abg_input_test.docx");
        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("word/document.xml", options).unwrap();
            use std::io::Write as _;
            writer.write_all(document.as_bytes()).unwrap();
            writer.finish().unwrap();
        }

        let chapters = parse_docx(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "First chapter");
        assert_eq!(chapters[0].text, "Body text one.");
        assert_eq!(chapters[1].title, "Second chapter");
        assert_eq!(chapters[1].text, "Body text & two.");
    }
}
