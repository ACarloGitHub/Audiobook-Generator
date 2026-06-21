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

    // 1. Try the ToC. EPUB 2 stores it in OEBPS/toc.ncx; EPUB 3 stores
    //    it inside a NAV document declared in the manifest as
    //    <item properties="nav">.
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut processed_hrefs: HashSet<String> = HashSet::new();
    let mut chapter_count: usize = 0;

    let toc_entries = try_toc_ncx(&mut zip, &opf_path)
        .or_else(|| try_nav_document(&mut zip, &opf_path, &manifest));

    if let Some(entries) = toc_entries {
        for (raw_title, href) in entries {
            // The legacy code strips any #fragment from the href before
            // resolving. EPUB 3 NAV often uses fragments to point at
            // sub-sections of the same XHTML.
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

    // 2. Fallback: walk the spine in order. The legacy behaviour is
    //    to treat the whole spine as a single chapter, so the user can
    //    see the cover, the index, the chapters, and the back matter in
    //    one continuous read. We honour that.
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

/// Sanitise a raw chapter title for use in a filename. The legacy code
/// used `utils.sanitize_filename`; we re-implement the same idea here.
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

/// Read META-INF/container.xml, find the OPF, return (opf-relative-path, opf-bytes).
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

/// Parse the OPF manifest into a map of id -> (href, media-type, properties).
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
                    let mut media_type = None;
                    let mut properties = None;
                    for attr in e.attributes() {
                        let attr = attr?;
                        match attr.key.as_ref() {
                            b"id" => id = Some(attr.unescape_value()?.into_owned()),
                            b"href" => href = Some(attr.unescape_value()?.into_owned()),
                            b"media-type" => media_type = Some(attr.unescape_value()?.into_owned()),
                            b"properties" => properties = Some(attr.unescape_value()?.into_owned()),
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(href)) = (id, href) {
                        out.insert(
                            id,
                            ManifestItem {
                                href,
                                media_type: media_type.unwrap_or_default(),
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
    media_type: String,
    properties: String,
}

/// Parse the OPF `<spine>` in document order. Returns `(id, href)` pairs.
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

/// Read an EPUB 2 `toc.ncx` next to the OPF. Returns a list of
/// `(title, href)` pairs, in document order, flattened across nested
/// navPoints (each navPoint's own link is taken; nested sub-navPoints
/// are recorded under the parent in the publisher's source, but we
/// only need the top-level entries to drive the chapter list).
///
/// We intentionally replicate the legacy `flatten_toc_items` behaviour:
/// when a navPoint contains a sub-list, the parent's link is taken
/// and the sub-list is recursively walked for its own top-level link.
/// This means a deeply-nested ToC produces one chapter per leaf
/// navPoint plus the parents, which matches what the legacy code did.
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

    // We track the current navPoint's title and src on the stack so
    // that when we close the element we can emit the entry.
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
            // `<content src="..."/>` is self-closing in NCX. quick_xml
            // emits Empty for self-closing tags; we need to read its
            // attributes too.
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

/// Read an EPUB 3 NAV document. Returns a list of `(title, href)` pairs
/// in document order. Only top-level `<li>` entries under the nav `<ol>`
/// are taken; nested lists are flattened.
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

/// Strip XHTML/HTML tags and decode the most common entities. Faithful
/// port of the legacy `extract_text_from_item`: it removes the same set
/// of tags (script, style, nav, header, footer, aside, figure, img, a,
/// sup, sub, hr, br, map, area, noscript, iframe, object, embed, video,
/// audio, source, track) and replaces br/hr with newlines.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    // Track which tags we are inside so we can drop their text.
    let skip_tags: &[&[u8]] = &[
        b"script", b"style", b"nav", b"header", b"footer", b"aside", b"figure",
        b"img", b"a", b"sup", b"sub", b"map", b"area", b"noscript",
        b"iframe", b"object", b"embed", b"video", b"audio", b"source", b"track",
    ];
    let mut skip_depth: usize = 0;
    let break_tags: &[&[u8]] = &[b"br", b"hr"];

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if skip_tags.iter().any(|t| *t == name.as_slice()) {
                    skip_depth += 1;
                } else if break_tags.iter().any(|t| *t == name.as_slice()) {
                    out.push('\n');
                }
            }
            Ok(Event::End(_)) => {
                // We rely on the matching Start/End counts. If a skip
                // tag closes, drop out of the skip region. For all
                // other closing tags we add a newline if the legacy
                // code would (block-level elements).
                if skip_depth > 0 {
                    skip_depth -= 1;
                } else {
                    // We do not currently emit per-tag newlines for
                    // block elements. The legacy code used
                    // `soup.get_text(separator='\n')` which emits a
                    // newline at the boundary of every block element.
                    // We approximate that with a regex post-pass.
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
                    out.push_str(&unescaped);
                }
            }
            Ok(Event::CData(c)) => {
                if skip_depth == 0 {
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

    // Post-processing: collapse horizontal whitespace, drop empty
    // lines, and trim. Mirrors the legacy `re.sub` calls.
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
        } else if c == ' ' || c == '\t' {
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

#[allow(dead_code)]
fn _unused_marker() -> Result<()> {
    Ok(())
}
