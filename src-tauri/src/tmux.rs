//! Native tmux-per-project session persistence.
//!
//! Reimplements the pattern claunch popularised (one detached tmux session per
//! project, named deterministically) without vendoring its script — claunch
//! defaults to `--dangerously-skip-permissions`, which this project does not
//! adopt. See CLAUDE.md rules 1 and 4.
//!
//! tmux is a hard dependency; there is no tmux-less direct mode.

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

/// Flags this project must never pass to `claude`. Enforced by test.
pub const FORBIDDEN_FLAGS: &[&str] = &["--dangerously-skip-permissions", "--permission-mode"];

/// Builds the argv used to start Claude Code inside a project's tmux session.
///
/// Deliberately minimal: no permission-mode flags at all, so the session
/// inherits Claude Code's own default (prompted). Per-project permission mode
/// is a settings-file concern handled in M4, never a flag smuggled in here.
pub fn claude_argv() -> Vec<String> {
    vec!["claude".to_string()]
}

/// Deterministic, tmux-legal session name for a project.
///
/// tmux treats '.' and ':' as target syntax, so they can never appear in a
/// name. The readable slug is for `tmux ls`; the hash suffix is what actually
/// guarantees uniqueness, and it is taken over the full `key` so two projects
/// with the same basename never collide.
pub fn session_name(key: &str) -> String {
    let slug: String = key
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(key)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let slug: String = slug.trim_matches('_').chars().take(24).collect();
    let slug = if slug.is_empty() { "project".to_string() } else { slug };
    format!("ccc-{slug}-{:08x}", fnv1a(key))
}

/// FNV-1a. Chosen over DefaultHasher because that one's output is explicitly
/// not stable across releases, and these names must survive upgrades.
fn fnv1a(s: &str) -> u32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    (h ^ (h >> 32)) as u32
}

pub fn is_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tmux(args: &[&str]) -> Result<std::process::Output> {
    Command::new("tmux").args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::TmuxMissing
        } else {
            Error::Io(e)
        }
    })
}

pub fn has_session(name: &str) -> Result<bool> {
    // `-t=` forces an exact match; without it tmux accepts prefixes and would
    // report a different project's session as ours.
    let out = tmux(&["has-session", "-t", &format!("={name}")])?;
    Ok(out.status.success())
}

pub fn list_sessions() -> Result<Vec<String>> {
    let out = tmux(&["list-sessions", "-F", "#{session_name}"])?;
    if !out.status.success() {
        // No server running yet is the normal empty case, not an error.
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|s| s.starts_with("ccc-"))
        .collect())
}

/// Creates the project's detached session if it doesn't exist. Idempotent.
/// Returns true if a session was created.
pub fn ensure_session(name: &str, cwd: &Path) -> Result<bool> {
    if has_session(name)? {
        return Ok(false);
    }
    if !cwd.is_dir() {
        return Err(Error::UnresolvedProject(cwd.display().to_string()));
    }

    let argv = claude_argv();
    debug_assert!(!argv.iter().any(|a| FORBIDDEN_FLAGS.contains(&a.as_str())));

    let mut args: Vec<&str> = vec!["new-session", "-d", "-s", name, "-c", cwd.to_str().unwrap_or(".")];
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    args.push("--");
    args.extend(argv_refs);

    let out = tmux(&args)?;
    if !out.status.success() {
        return Err(Error::Tmux(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(true)
}

pub fn kill_session(name: &str) -> Result<()> {
    let out = tmux(&["kill-session", "-t", &format!("={name}")])?;
    if !out.status.success() {
        return Err(Error::Tmux(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(())
}

/// The command the user runs to attach. Returned for display/clipboard rather
/// than executed — attaching needs a real terminal, which the GUI is not.
pub fn attach_command(name: &str) -> String {
    format!("tmux attach -t {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_passes_a_permission_bypass_flag() {
        // CLAUDE.md rule 1, enforced rather than merely intended.
        let argv = claude_argv();
        for flag in FORBIDDEN_FLAGS {
            assert!(!argv.iter().any(|a| a == flag), "argv leaked {flag}: {argv:?}");
        }
        assert!(!argv.join(" ").contains("dangerously"));
    }

    #[test]
    fn session_name_is_deterministic() {
        let a = session_name("/run/media/x/proj");
        assert_eq!(a, session_name("/run/media/x/proj"));
    }

    #[test]
    fn session_name_is_tmux_legal() {
        for key in ["/a/b.c/pro:ject", "-run-media-x-The-Plan-zen", "/", ""] {
            let n = session_name(key);
            assert!(!n.contains('.'), "{n}");
            assert!(!n.contains(':'), "{n}");
            assert!(!n.starts_with('-'), "{n}");
            assert!(!n.is_empty());
        }
    }

    #[test]
    fn same_basename_different_projects_do_not_collide() {
        assert_ne!(session_name("/home/a/api"), session_name("/home/b/api"));
    }

    #[test]
    fn slug_stays_readable() {
        assert!(session_name("/run/media/x/cc-console").starts_with("ccc-cc_console-"));
    }
}
