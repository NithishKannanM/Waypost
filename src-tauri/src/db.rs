use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Migrations are embedded so a built binary never depends on files next to it.
/// Index in this array + 1 == the `user_version` the migration leaves behind.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/001_init.sql")];

pub fn data_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs_home().map(|h| h.join(".local/share")))
        .ok_or(Error::NoHome)?;
    Ok(base.join("cc-console"))
}

pub fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

pub fn open_default() -> Result<Connection> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir)?;
    open_at(&dir.join("cc-console.db"))
}

pub fn open_at(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

/// Applies any migration the database hasn't seen yet, in order, each inside a
/// transaction. Idempotent: re-running against an up-to-date database is a
/// no-op. Never edits an already-applied migration's effects — new schema
/// changes are new files.
pub fn migrate(conn: &Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let current = current.max(0) as usize;

    for (idx, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let version = idx + 1;
        conn.execute_batch(&format!(
            "BEGIN;\n{sql}\nPRAGMA user_version = {version};\nCOMMIT;"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn migration_creates_prd_schema() {
        let conn = mem();
        let mut names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'findings_fts_%'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        names.sort();
        assert_eq!(
            names,
            vec![
                "bypass_sessions",
                "findings",
                "findings_fts",
                "projects",
                "sessions",
                "skills",
                "summaries",
            ]
        );
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = mem();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn findings_source_rejects_mcp_until_m6() {
        // Guards the M6 migration: 'mcp' must be added deliberately in 002,
        // not silently allowed here.
        let conn = mem();
        conn.execute(
            "INSERT INTO sessions VALUES ('s','/p','/p/t.jsonl','now','now',NULL)",
            [],
        )
        .unwrap();
        let err = conn.execute(
            "INSERT INTO findings (session_id, project_path, finding, captured_at, source)
             VALUES ('s','/p','f','now','mcp')",
            [],
        );
        assert!(err.is_err());
    }

    #[test]
    fn fts5_is_available() {
        let conn = mem();
        conn.execute("INSERT INTO findings_fts (rowid, finding) VALUES (1, 'tmux socket path')", [])
            .unwrap();
        let hits: i64 = conn
            .query_row("SELECT count(*) FROM findings_fts WHERE findings_fts MATCH 'tmux'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(hits, 1);
    }
}
