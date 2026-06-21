use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use tracing::warn;

/// One chapter of an EPUB, already stripped of HTML.
pub struct Chapter {
    pub title: String,
    pub text: String,
}

/// Open an EPUB, walk its spine, and return one entry per content document.
///
/// Minimal implementation: enough for the prototype to validate the
/// pipeline end-to-end. We use `zip` to read the archive and `quick-xml`
/// to parse the OPF and each XHTML document. We do not try to honour
/// every EPUB 2/3 quirk; if the input is malformed in non-trivial ways
/// the parser will give up with a clear error.
pub fn extract_chapters(path: &Path) -> Result<Vec<Chapter>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid ZIP / EPUB", path.display()))?;

    let (opf_path, opf_bytes) = read_container_xml(&mut zip)?;
    let opf = String::from_utf8(opf_bytes).context("OPF is not UTF-8")?;
    let spine = parse_opf_spine(&opf).with_context(|| format!("parsing {opf_path}"))?;

    let mut chapters = Vec::new();
    for (i, (id, href)) in spine.into_iter().enumerate() {
        let inner_path = resolve_path(&opf_path, &href);
        let bytes = read_zip_entry(&mut zip, &inner_path)
            .with_context(|| format!("reading spine item {i} at {inner_path}"))?;
        let xhtml = String::from_utf8(bytes).context("spine item is not UTF-8")?;
        let text = html_to_text(&xhtml);
        if text.trim().is_empty() {
            warn!("spine item {i} ({id}) is empty after HTML stripping, skipping");
            continue;
        }
        let title = title_from_id(&id, i + 1);
        chapters.push(Chapter { title, text });
    }

    if chapters.is_empty() {
        bail!("EPUB contained no readable text content");
    }

    Ok(chapters)
}

/// Read META-INF/container.xml, find the OPF, return (opf-relative-path, opf-bytes).
fn read_container_xml<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<(String, Vec<u8>)> {
    let mut container = String::new();
    zip.by_name("META-INF/container.xml")
        .context("missing META-INF/container.xml — not a valid EPUB")?
        .read_to_string(&mut container)
        .context("reading container.xml")?;

    // Look for <rootfile full-path="..." media-type="application/oebps-package+xml"/>
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

/// Parse `<spine><itemref idref="..." /></spine>` plus the matching
/// `<manifest><item id="..." href="..." /></manifest>` to resolve hrefs.
fn parse_opf_spine(opf: &str) -> Result<Vec<(String, String)>> {
    let mut reader = Reader::from_str(opf);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    // First pass: collect the manifest id -> href map
    let mut manifest: std::collections::HashMap<String, String> = Default::default();
    let mut in_manifest = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"manifest" => in_manifest = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"manifest" => in_manifest = false,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if in_manifest => {
                if e.name().as_ref() == b"item" {
                    let mut id = None;
                    let mut href = None;
                    for attr in e.attributes() {
                        let attr = attr?;
                        match attr.key.as_ref() {
                            b"id" => id = Some(attr.unescape_value()?.into_owned()),
                            b"href" => href = Some(attr.unescape_value()?.into_owned()),
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(href)) = (id, href) {
                        manifest.insert(id, href);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("OPF: {e}"),
            _ => {}
        }
        buf.clear();
    }

    // Second pass: walk the spine in document order
    let mut spine: Vec<(String, String)> = Vec::new();
    let mut in_spine = false;
    buf.clear();
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
                            if let Some(href) = manifest.get(&id) {
                                spine.push((id, href.clone()));
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("OPF: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(spine)
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

/// Resolve a relative href against the OPF's directory.
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

fn title_from_id(id: &str, fallback_index: usize) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { ' ' })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        format!("Chapter {fallback_index:02}")
    } else {
        cleaned.to_string()
    }
}

/// Strip XHTML/HTML tags and decode the most common entities. We do not
/// try to render CSS or interpret `<script>` / `<style>` — we just drop
/// their text content if it ever appears in a chapter.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_skip = false;
    let mut skip_tag_name: Option<Vec<u8>> = None;

    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if matches!(name.as_slice(), b"script" | b"style") {
                    in_skip = true;
                    skip_tag_name = Some(name);
                } else {
                    in_tag = true;
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                if in_skip && skip_tag_name.as_deref() == Some(name.as_slice()) {
                    in_skip = false;
                    skip_tag_name = None;
                } else {
                    in_tag = false;
                }
                if matches!(
                    name.as_slice(),
                    b"p" | b"div" | b"br" | b"li" | b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6"
                ) {
                    out.push('\n');
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                if matches!(name.as_slice(), b"br" | b"hr" | b"img") {
                    out.push('\n');
                }
            }
            Ok(Event::Text(t)) => {
                if !in_skip {
                    let unescaped = t.unescape().unwrap_or_default();
                    out.push_str(&unescaped);
                }
            }
            Ok(Event::CData(c)) => {
                if !in_skip {
                    out.push_str(std::str::from_utf8(c.as_ref()).unwrap_or(""));
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

    // Collapse whitespace runs
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_space = false;
    let mut prev_newline = false;
    for c in out.chars() {
        if c == '\n' {
            if !prev_newline {
                collapsed.push('\n');
            }
            prev_newline = true;
            prev_space = false;
        } else if c.is_whitespace() {
            if !prev_space && !prev_newline {
                collapsed.push(' ');
            }
            prev_space = true;
            prev_newline = false;
        } else {
            collapsed.push(c);
            prev_space = false;
            prev_newline = false;
        }
    }

    collapsed.trim().to_string()
}

// Quiet the unused warning if we don't end up calling FromStr anywhere.
#[allow(dead_code)]
fn _force_fromstr_use(s: &str) -> Result<()> {
    let _ = String::from_str(s).map_err(|_| anyhow!("fromstr fail"));
    Ok(())
}
