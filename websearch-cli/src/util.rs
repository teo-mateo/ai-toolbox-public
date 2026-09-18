use std::io::Write;

use crate::search::{ExaResponse, SerpapiResponse};

/// Sanitize a query string for use in filenames.
fn sanitize_filename(query: &str) -> String {
    let sanitized = query
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized
    }
}

/// Resolve the log directory.
///
/// The Python server logs to `logs/` at the project root. The CLI binary lives
/// at `<project>/cli/target/<profile>/websearch-cli`. We walk up from the
/// binary directory looking for a `logs/` directory, then fall back to
/// `logs/` relative to CWD.
fn resolve_log_dir() -> Option<std::path::PathBuf> {
    // Walk up from the binary looking for a `logs/` directory.
    if let Ok(exe) = std::env::current_exe() {
        let mut current = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..10 {
            match current {
                Some(ref dir) if dir.join("logs").is_dir() => return Some(dir.join("logs")),
                Some(dir) => current = dir.parent().map(|p| p.to_path_buf()),
                None => break,
            }
        }
    }
    // Fallback: logs/ relative to CWD
    std::env::current_dir().ok().map(|d| d.join("logs"))
}

/// Log a SerpAPI response to `logs/{timestamp}_{engine}_{query}.json`.
/// Silently ignores all errors.
pub fn log_serpapi_response(query: &str, limit: usize, data: &SerpapiResponse) {
    let log_dir = match resolve_log_dir() {
        Some(d) => d,
        None => return,
    };

    let ts = format_timestamp();
    let safe_query = sanitize_filename(query);
    let filename = format!("{}_google_{}.json", ts, safe_query);

    let log_entry = serde_json::json!({
        "timestamp": ts,
        "query": query,
        "params": {
            "engine": "google",
            "num": limit,
        },
        "response": data,
    });

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_dir.join(&filename))
    {
        let _ = writeln!(f, "{}", serde_json::to_string_pretty(&log_entry).unwrap_or_default());
    }
}

/// Log an Exa response to `logs/{timestamp}_exa_{query}.json`.
/// Silently ignores all errors.
pub fn log_exa_response(query: &str, data: &ExaResponse) {
    let log_dir = match resolve_log_dir() {
        Some(d) => d,
        None => return,
    };

    let ts = format_timestamp();
    let safe_query = sanitize_filename(query);
    let filename = format!("{}_exa_{}.json", ts, safe_query);

    let log_entry = serde_json::json!({
        "timestamp": ts,
        "query": query,
        "response": data,
    });

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_dir.join(&filename))
    {
        let _ = writeln!(f, "{}", serde_json::to_string_pretty(&log_entry).unwrap_or_default());
    }
}

/// Format current time as `YYYY-MM-DDTHH-MM-SS` matching the Python server's
/// `datetime.datetime.now().strftime("%Y-%m-%dT%H-%M-%S")` (local time).
fn format_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}
