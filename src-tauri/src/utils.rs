use regex::Regex;
use std::sync::OnceLock;

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

/// Build a useful error detail from a failed helper process: the last
/// non-empty stderr lines, falling back to stdout when stderr says
/// nothing. Taking only the very last line used to drop the actual
/// reason (e.g. "[CLI] ERROR: <reason>" spans two lines).
pub fn process_error_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let pick = |bytes: &[u8]| -> String {
        let text = String::from_utf8_lossy(bytes);
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let start = lines.len().saturating_sub(5);
        lines[start..].join(" | ")
    };
    let err = pick(stderr);
    if !err.is_empty() {
        return err;
    }
    pick(stdout)
}
