use regex::Regex;
use std::sync::OnceLock;

pub fn count_words_proxy(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn sanitize_filename(name: &str) -> String {
    static INVALID_CHARS: OnceLock<Regex> = OnceLock::new();
    static WHITESPACE: OnceLock<Regex> = OnceLock::new();
    static MULTI_DOT: OnceLock<Regex> = OnceLock::new();

    let invalid = INVALID_CHARS.get_or_init(|| Regex::new(r#"[\\/*?:"<>|']"#).unwrap());
    let ws = WHITESPACE.get_or_init(|| Regex::new(r"\s+").unwrap());
    let dots = MULTI_DOT.get_or_init(|| Regex::new(r"\.{2,}").unwrap());

    let step1 = invalid.replace_all(name, "").to_string();
    let step2 = ws.replace_all(&step1, "_").to_string();
    let step3 = dots.replace_all(&step2, ".").to_string();
    let step4 = step3.trim_matches(|c: char| c == '.' || c == '_' || c == ' ').to_string();

    if step4.is_empty() {
        return "invalid_title".to_string();
    }
    if step4.len() > 100 {
        let mut truncated = step4[..100].to_string();
        truncated = truncated.trim_end_matches(|c: char| c == '.' || c == '_' || c == '-').to_string();
        if truncated.is_empty() {
            return "invalid_title".to_string();
        }
        truncated
    } else {
        step4
    }
}

pub fn replace_guillemets_text(text: &str) -> String {    text.replace('\u{00AB}', "\"").replace('\u{00BB}', "\"")
}

/// On Windows, prevent a console window from flashing open when spawning a
/// console-subsystem helper (llama-server, ffmpeg, ...) from the GUI app.
/// No-op on other platforms. Same flag as wizard::silent_command.
pub fn hide_console_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

#[derive(Debug, Clone)]
pub struct DialogueLine {
    pub actor: String,
    pub text: String,
}

pub fn parse_dialogue_script(script_text: &str) -> Vec<DialogueLine> {
    if script_text.is_empty() {
        return Vec::new();
    }
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let re = PATTERN.get_or_init(|| Regex::new(r"\[([a-zA-Z0-9_\s]+)\]").unwrap());

    let parts: Vec<&str> = re.split(script_text).collect();
    let matches: Vec<&str> = re.find_iter(script_text).map(|m| m.as_str()).collect();

    let mut result = Vec::new();
    let mut match_idx = 0;

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if match_idx < matches.len() {
            let actor = matches[match_idx]
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            let text = part.trim().to_string();
            if !text.is_empty() {
                result.push(DialogueLine { actor, text });
            }
            match_idx += 1;
        }
    }

    result
}