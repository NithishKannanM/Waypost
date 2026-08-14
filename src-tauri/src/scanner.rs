//! Transcript metadata scanner.
//!
//! Reads exactly what CLAUDE.md rule 2 permits: the filename (session id), the
//! file mtime, and a best-effort preview drawn from the head of the file.
//! Nothing here depends on Claude Code's JSONL field names — see `preview`.
//!
//! Glob is `~/.claude/projects/*/*.jsonl`, deliberately non-recursive. Project
//! directories also contain UUID-named sidecar directories holding
//! `subagents/*.jsonl`; those are not top-level sessions and recursing would
//! mix subagent runs into the session list.
//!
//! Resilience rule: any unreadable file or directory is skipped. A malformed
//! transcript never aborts a scan.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::db::dirs_home;
use crate::error::{Error, Result};

/// Lines inspected while looking for a usable preview.
const PREVIEW_SCAN_LINES: usize = 20;
/// Hard cap on bytes read per transcript, so a multi-MB session stays cheap.
const PREVIEW_SCAN_BYTES: u64 = 256 * 1024;
const PREVIEW_MAX_CHARS: usize = 200;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScannedSession {
    pub session_id: String,
    pub dir_name: String,
    pub transcript_path: PathBuf,
    pub last_active_at: String,
    pub preview: Option<String>,
}

pub fn projects_root() -> Result<PathBuf> {
    Ok(dirs_home().ok_or(Error::NoHome)?.join(".claude/projects"))
}

pub fn scan_default() -> Result<Vec<ScannedSession>> {
    Ok(scan(&projects_root()?))
}

/// Enumerates sessions under `root`. Never fails as a whole: unreadable
/// entries are skipped individually.
pub fn scan(root: &Path) -> Vec<ScannedSession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for project in entries.flatten() {
        if !project.path().is_dir() {
            continue;
        }
        let dir_name = project.file_name().to_string_lossy().into_owned();

        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(session_id) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            let Ok(meta) = file.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }

            let last_active_at = meta
                .modified()
                .map(|t| DateTime::<Utc>::from(t).to_rfc3339())
                .unwrap_or_else(|_| Utc::now().to_rfc3339());

            out.push(ScannedSession {
                session_id,
                dir_name: dir_name.clone(),
                preview: preview(&path),
                transcript_path: path,
                last_active_at,
            });
        }
    }
    out
}

/// Best-effort preview, strategy (c): scan the head of the file for the first
/// line containing human-readable prose, identified *structurally* rather than
/// by field name — we walk the JSON value tree and take the longest string
/// that reads like prose. When a line yields nothing we move to the next.
///
/// This is deliberately not bound to `type`/`message`/`content`: when the
/// upstream schema shifts, the preview degrades to the raw first line instead
/// of breaking. Preview is cosmetic and is allowed to be `None`.
fn preview(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file.take(PREVIEW_SCAN_BYTES));

    let mut first_raw: Option<String> = None;
    let mut line = String::new();

    for _ in 0..PREVIEW_SCAN_LINES {
        line.clear();
        let mut bytes = Vec::new();
        if reader.read_until(b'\n', &mut bytes).ok()? == 0 {
            break;
        }
        line.push_str(&String::from_utf8_lossy(&bytes));
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if first_raw.is_none() {
            first_raw = Some(truncate(trimmed));
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            if let Some(text) = longest_prose(&v) {
                return Some(truncate(&text));
            }
        }
    }

    // Nothing prose-like in the scan window: fall back to the literal first
    // line, which is what the PRD specifies as the baseline behaviour.
    first_raw
}

fn longest_prose(v: &Value) -> Option<String> {
    let mut best: Option<&str> = None;
    collect(v, &mut best);
    best.map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn collect<'a>(v: &'a Value, best: &mut Option<&'a str>) {
    match v {
        Value::String(s) => {
            if is_prose(s) && best.map_or(true, |b| s.len() > b.len()) {
                *best = Some(s);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect(x, best)),
        Value::Object(o) => o.values().for_each(|x| collect(x, best)),
        _ => {}
    }
}

/// Structural prose test — no field names involved.
fn is_prose(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 8 || s.len() > 4000 {
        return false;
    }
    if is_uuid(s) || is_timestamp(s) {
        return false;
    }
    let has_space = s.contains(char::is_whitespace);
    // Unbroken long runs are hashes, tokens, or base64 blobs, not prose.
    if !has_space {
        return false;
    }
    // Filesystem paths and command lines masquerade as prose less usefully.
    if s.starts_with('/') && !s.contains(". ") {
        return false;
    }
    s.chars().filter(|c| c.is_alphabetic()).count() * 2 > s.len()
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.as_bytes()
            .iter()
            .enumerate()
            .all(|(i, b)| match i {
                8 | 13 | 18 | 23 => *b == b'-',
                _ => b.is_ascii_hexdigit(),
            })
}

fn is_timestamp(s: &str) -> bool {
    s.len() >= 10
        && s.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        && s.as_bytes()[4] == b'-'
}

fn truncate(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= PREVIEW_MAX_CHARS {
        return s.to_string();
    }
    s.chars().take(PREVIEW_MAX_CHARS).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("-run-media-x-proj");
        std::fs::create_dir_all(&proj).unwrap();

        // Real-world shape: bookkeeping lines precede the first user message.
        let mut f = File::create(proj.join("aaaaaaaa-0000-0000-0000-000000000001.jsonl")).unwrap();
        writeln!(f, r#"{{"type":"queue-operation","operation":"enqueue","timestamp":"2026-07-18T15:22:08.401Z","sessionId":"3157a605-c544-4a97-ac87-09fb8d6df249"}}"#).unwrap();
        writeln!(f, r#"{{"type":"queue-operation","operation":"dequeue","timestamp":"2026-07-18T15:22:08.401Z"}}"#).unwrap();
        writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"load the context we have some small work"}}]}}}}"#).unwrap();

        // Malformed file must not abort the scan.
        let mut bad = File::create(proj.join("aaaaaaaa-0000-0000-0000-000000000002.jsonl")).unwrap();
        writeln!(bad, "{{not json at all").unwrap();

        std::fs::write(proj.join("empty.jsonl"), b"").unwrap();
        std::fs::write(proj.join("notes.txt"), b"ignored").unwrap();

        // Sidecar subagent transcripts must be excluded by the non-recursive glob.
        let sub = proj.join("aaaaaaaa-0000-0000-0000-000000000001/subagents");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("sub-1.jsonl"), b"{}\n").unwrap();

        tmp
    }

    #[test]
    fn finds_top_level_sessions_only() {
        let tmp = fixture();
        let found = scan(tmp.path());
        let mut ids: Vec<_> = found.iter().map(|s| s.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "aaaaaaaa-0000-0000-0000-000000000001",
                "aaaaaaaa-0000-0000-0000-000000000002",
                "empty",
            ]
        );
        assert!(!found.iter().any(|s| s.session_id == "sub-1"), "subagent transcript leaked in");
    }

    #[test]
    fn preview_skips_bookkeeping_lines() {
        let tmp = fixture();
        let found = scan(tmp.path());
        let s = found
            .iter()
            .find(|s| s.session_id.ends_with("001"))
            .unwrap();
        assert_eq!(s.preview.as_deref(), Some("load the context we have some small work"));
    }

    #[test]
    fn malformed_file_does_not_abort_scan_and_falls_back() {
        let tmp = fixture();
        let found = scan(tmp.path());
        let s = found.iter().find(|s| s.session_id.ends_with("002")).unwrap();
        // Falls back to the raw first line rather than failing.
        assert_eq!(s.preview.as_deref(), Some("{not json at all"));
    }

    #[test]
    fn empty_file_has_no_preview() {
        let tmp = fixture();
        let found = scan(tmp.path());
        let s = found.iter().find(|s| s.session_id == "empty").unwrap();
        assert_eq!(s.preview, None);
    }

    #[test]
    fn missing_root_yields_empty_not_error() {
        assert!(scan(Path::new("/nonexistent/cc-console/test")).is_empty());
    }

    #[test]
    fn prose_test_rejects_ids_and_timestamps() {
        assert!(!is_prose("3157a605-c544-4a97-ac87-09fb8d6df249"));
        assert!(!is_prose("2026-07-18T15:22:08.401Z"));
        assert!(!is_prose("enqueue"));
        assert!(is_prose("load the context we have some small work"));
    }
}
