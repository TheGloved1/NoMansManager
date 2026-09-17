//! Persistent activity log: every backend operation is recorded here.
//!
//! One JSON object per line (`{"ts","level","source","msg"}`), newest last.
//! The file is capped (default 2000 lines, adjustable in Settings); an
//! overflowing append keeps only the newest lines. Readers tolerate corrupt
//! lines by skipping them.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_LINES: usize = 2000;
const MAX_MESSAGE_CHARS: usize = 2000;
const STORE_KEY_MAX_LINES: &str = "logs_max_lines";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LogEntry {
    pub ts: String,
    pub level: String,
    pub source: String,
    pub msg: String,
}

fn log_file() -> PathBuf {
    log_dir().join("app.log")
}

#[tauri::command]
pub(crate) fn get_logs_dir() -> Result<String, String> {
    let dir = log_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

fn log_dir() -> PathBuf {
    super::app_data_dir().join("logs")
}

/// Retention cap: Settings value if present and sane, else the default.
fn max_lines() -> usize {
    let store_file = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dev.gloved.nomansmanager")
        .join("settings.json");
    if let Ok(text) = fs::read_to_string(store_file) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(n) = v.get(STORE_KEY_MAX_LINES).and_then(|n| n.as_u64()) {
                return (n as usize).clamp(100, 100_000);
            }
        }
    }
    DEFAULT_MAX_LINES
}

fn sanitize_entry(level: &str, source: &str, message: &str) -> LogEntry {
    let level = match level {
        "warn" | "error" => level.to_string(),
        _ => "info".to_string(),
    };
    let msg: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
    LogEntry {
        ts: chrono::Local::now().to_rfc3339(),
        level,
        source: source.chars().take(32).collect(),
        msg,
    }
}

fn append_to_log(path: &Path, max_lines: usize, entry: &LogEntry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect();
    lines.push(line);
    if lines.len() > max_lines {
        let drop = lines.len() - max_lines;
        lines.drain(..drop);
        fs::write(path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    } else {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        writeln!(f, "{}", lines.last().expect("just pushed")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_log_file(path: &Path) -> Vec<LogEntry> {
    let text = fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| serde_json::from_str::<LogEntry>(line).ok())
        .collect()
}

/// Direct backend logging for command internals (step detail the frontend
/// wrapper can't see). Fire-and-forget: logging never fails the operation.
pub(crate) fn dlog(app: &tauri::AppHandle, level: &str, source: &str, msg: String) {
    use tauri::Emitter;
    let entry = sanitize_entry(level, source, &msg);
    let _ = append_to_log(&log_file(), max_lines(), &entry);
    let _ = app.emit("logs-changed", &());
}

#[tauri::command]
pub(crate) fn append_log(
    app: tauri::AppHandle,
    level: String,
    source: String,
    message: String,
) -> Result<(), String> {
    let entry = sanitize_entry(&level, &source, &message);
    let res = append_to_log(&log_file(), max_lines(), &entry);
    // Notify log viewers; a failed emit must not fail the logged action.
    use tauri::Emitter;
    let _ = app.emit("logs-changed", &());
    res
}

#[tauri::command]
pub(crate) fn read_logs() -> Result<Vec<LogEntry>, String> {
    Ok(read_log_file(&log_file()))
}

#[tauri::command]
pub(crate) fn clear_logs() -> Result<(), String> {
    let path = log_file();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_log(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nmm_logtest_{}_{}", std::process::id(), tag));
        let _ = fs::remove_file(&p);
        p
    }

    fn entry(i: usize) -> LogEntry {
        LogEntry {
            ts: format!("t{i}"),
            level: "info".into(),
            source: "test".into(),
            msg: format!("message {i}"),
        }
    }

    #[test]
    fn rotation_keeps_newest_lines() {
        let p = tmp_log("rot");
        for i in 0..10 {
            append_to_log(&p, 4, &entry(i)).unwrap();
        }
        let got = read_log_file(&p);
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].msg, "message 6");
        assert_eq!(got[3].msg, "message 9");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn sanitize_truncates_and_normalizes() {
        let e = sanitize_entry("bogus", "a-very-long-source-name-that-gets-cut-off-here", &"x".repeat(5000));
        assert_eq!(e.level, "info");
        assert!(e.source.len() <= 32);
        assert_eq!(e.msg.chars().count(), MAX_MESSAGE_CHARS);
        let e2 = sanitize_entry("error", "s", "boom");
        assert_eq!(e2.level, "error");
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let p = tmp_log("corrupt");
        fs::write(&p, "not json\n{\"ts\":\"t\",\"level\":\"info\",\"source\":\"s\",\"msg\":\"ok\"}\n{{{bad\n").unwrap();
        let got = read_log_file(&p);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].msg, "ok");
        let _ = fs::remove_file(&p);
    }
}
