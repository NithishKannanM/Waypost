//! Recovering a real cwd from a `~/.claude/projects/` directory name.
//!
//! Claude Code encodes the project cwd by replacing every character that isn't
//! alphanumeric with '-'. Verified against real data: `/` , `_` and ' ' all
//! collapse to the same '-', so `The Plan` encodes as `The-Plan` and
//! `sleep_apnea_detection` as `sleep-apnea-detection`. The encoding is
//! many-to-one and cannot be inverted by string manipulation.
//!
//! So we don't invert it — we search. Descend from '/', and at each level read
//! the real directory and keep entries whose *own* encoding matches the next
//! run of tokens. One readdir per level, no combinatorial guessing, and it
//! handles every character class the encoder collapses rather than just '/'.
//!
//! No transcript content is read here: this stays inside CLAUDE.md rule 2.
//! A project whose directory was moved, deleted, or left on an unmounted
//! volume resolves to `Missing`, and callers must degrade rather than
//! fabricate a path.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "path")]
pub enum Resolution {
    /// Exactly one directory on disk encodes to this name.
    Resolved(PathBuf),
    /// Several do; we refuse to guess between them.
    Ambiguous,
    /// None does.
    Missing,
}

impl Resolution {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Resolution::Resolved(p) => Some(p.as_path()),
            _ => None,
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Resolution::Resolved(_) => "resolved",
            Resolution::Ambiguous => "ambiguous",
            Resolution::Missing => "missing",
        }
    }
}

/// Cap on directory reads per resolution, so a pathological tree can't stall a
/// scan. Exceeding it yields `Ambiguous` — unknown, not proven absent.
const READ_BUDGET: u32 = 4_000;

/// The encoder's transform: everything non-alphanumeric becomes '-'.
pub fn encode(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Lists subdirectory names of `dir`. Unreadable directories yield nothing
/// rather than failing the whole resolution.
fn real_subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

pub fn resolve(dir_name: &str) -> Resolution {
    resolve_with(dir_name, &real_subdirs)
}

pub fn resolve_with(dir_name: &str, list: &dyn Fn(&Path) -> Vec<String>) -> Resolution {
    let tokens: Vec<&str> = dir_name.split('-').collect();
    // A leading '-' is the encoded root '/'; drop the empty token it produces.
    let tokens: &[&str] = if tokens.first() == Some(&"") { &tokens[1..] } else { &tokens[..] };

    if tokens.is_empty() {
        return Resolution::Missing;
    }

    let mut found = Vec::new();
    let mut budget = READ_BUDGET;
    let exhausted = walk(Path::new("/"), tokens, list, &mut found, &mut budget);

    match found.len() {
        1 => Resolution::Resolved(found.pop().unwrap()),
        0 if exhausted => Resolution::Ambiguous,
        0 => Resolution::Missing,
        _ => Resolution::Ambiguous,
    }
}

/// Returns true if the read budget ran out (search incomplete).
fn walk(
    base: &Path,
    tokens: &[&str],
    list: &dyn Fn(&Path) -> Vec<String>,
    found: &mut Vec<PathBuf>,
    budget: &mut u32,
) -> bool {
    if tokens.is_empty() {
        found.push(base.to_path_buf());
        return false;
    }
    if *budget == 0 {
        return true;
    }
    *budget -= 1;

    let entries = list(base);

    // A directory name may itself contain the separator character, so one entry
    // can consume several tokens. Match on the entry's own encoding.
    for entry in entries {
        let encoded = encode(&entry);
        let take = encoded.split('-').count();
        if take > tokens.len() {
            continue;
        }
        if encoded != tokens[..take].join("-") {
            continue;
        }
        if walk(&base.join(&entry), &tokens[take..], list, found, budget) {
            return true;
        }
        // Two decodings is already ambiguous; stop enumerating.
        if found.len() > 1 {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fake(tree: &[(&str, &[&str])]) -> impl Fn(&Path) -> Vec<String> {
        let map: HashMap<PathBuf, Vec<String>> = tree
            .iter()
            .map(|(d, kids)| {
                (PathBuf::from(d), kids.iter().map(|s| s.to_string()).collect())
            })
            .collect();
        move |p: &Path| map.get(p).cloned().unwrap_or_default()
    }

    #[test]
    fn encode_matches_observed_behaviour() {
        assert_eq!(encode("The Plan"), "The-Plan");
        assert_eq!(encode("sleep_apnea_detection"), "sleep-apnea-detection");
        assert_eq!(encode("cc-console"), "cc-console");
        assert_eq!(encode("pia.phase1"), "pia-phase1");
    }

    #[test]
    fn resolves_unambiguous_path() {
        let fs = fake(&[("/", &["run"]), ("/run", &["media"]), ("/run/media", &["proj"])]);
        assert_eq!(
            resolve_with("-run-media-proj", &fs),
            Resolution::Resolved(PathBuf::from("/run/media/proj"))
        );
    }

    #[test]
    fn resolves_directory_containing_a_space() {
        let fs = fake(&[("/", &["run"]), ("/run", &["The Plan"]), ("/run/The Plan", &["zen"])]);
        assert_eq!(
            resolve_with("-run-The-Plan-zen", &fs),
            Resolution::Resolved(PathBuf::from("/run/The Plan/zen"))
        );
    }

    #[test]
    fn resolves_directory_containing_underscores() {
        let fs = fake(&[("/", &["home"]), ("/home", &["sleep_apnea_detection"])]);
        assert_eq!(
            resolve_with("-home-sleep-apnea-detection", &fs),
            Resolution::Resolved(PathBuf::from("/home/sleep_apnea_detection"))
        );
    }

    #[test]
    fn resolves_directory_containing_literal_hyphen() {
        let fs = fake(&[("/", &["run"]), ("/run", &["cc-console"])]);
        assert_eq!(
            resolve_with("-run-cc-console", &fs),
            Resolution::Resolved(PathBuf::from("/run/cc-console"))
        );
    }

    #[test]
    fn reports_ambiguous_when_two_encodings_collide() {
        // "a b" and "a-b" both encode to "a-b".
        let fs = fake(&[("/", &["a b", "a-b"]), ("/a b", &["z"]), ("/a-b", &["z"])]);
        assert_eq!(resolve_with("-a-b-z", &fs), Resolution::Ambiguous);
    }

    #[test]
    fn reports_missing_when_nothing_exists() {
        let fs = fake(&[("/", &[])]);
        assert_eq!(resolve_with("-gone-project", &fs), Resolution::Missing);
    }

    #[test]
    fn missing_is_not_confused_with_ambiguous() {
        let fs = fake(&[("/", &["run"]), ("/run", &["media"])]);
        assert_eq!(resolve_with("-run-media-nope", &fs), Resolution::Missing);
    }

    #[test]
    fn never_fabricates_a_path_for_unresolved() {
        let fs = fake(&[("/", &[])]);
        assert!(resolve_with("-x-y", &fs).path().is_none());
    }
}
