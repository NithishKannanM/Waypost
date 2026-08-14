use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::Connection;
use tauri::State;

use crate::error::{Error, Result};
use crate::project_path::{self, Resolution};
use crate::{scanner, tmux};

pub struct AppState {
    pub db: Mutex<Connection>,
}

#[derive(serde::Serialize)]
pub struct SessionView {
    pub session_id: String,
    pub dir_name: String,
    /// `None` when the project directory could not be resolved. The UI must
    /// not show a path here that we couldn't verify exists.
    pub project_path: Option<String>,
    pub resolution: String,
    pub transcript_path: String,
    pub last_active_at: String,
    pub preview: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ProjectView {
    pub dir_name: String,
    pub project_path: Option<String>,
    pub resolution: String,
    pub session_count: usize,
    pub last_active_at: String,
    pub tmux_session: String,
    pub tmux_running: bool,
}

#[derive(serde::Serialize)]
pub struct ScanSummary {
    pub projects: usize,
    pub sessions: usize,
    pub unresolved: usize,
}

/// Scans the transcript tree and upserts what it finds. Resolution is cached
/// in `projects` so the filesystem probe doesn't rerun for known directories,
/// but a previously-missing project is re-probed in case a volume got mounted.
#[tauri::command]
pub fn refresh(state: State<'_, AppState>) -> Result<ScanSummary> {
    let sessions = scanner::scan_default()?;
    let mut conn = state.db.lock().unwrap();
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();

    let mut cache: HashMap<String, Resolution> = HashMap::new();
    for s in &sessions {
        if cache.contains_key(&s.dir_name) {
            continue;
        }
        let cached: Option<(Option<String>, String)> = tx
            .query_row(
                "SELECT resolved_path, resolution FROM projects WHERE dir_name = ?1",
                [&s.dir_name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();

        // Only 'resolved' is trusted from cache; missing/ambiguous are cheap to
        // recheck and can legitimately change (unmounted drive, renamed dir).
        let res = match cached {
            Some((Some(p), ref k)) if k == "resolved" && std::path::Path::new(&p).is_dir() => {
                Resolution::Resolved(p.into())
            }
            _ => project_path::resolve(&s.dir_name),
        };
        cache.insert(s.dir_name.clone(), res);
    }

    for (dir_name, res) in &cache {
        tx.execute(
            "INSERT INTO projects (dir_name, resolved_path, resolution, last_seen_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(dir_name) DO UPDATE SET
               resolved_path = excluded.resolved_path,
               resolution    = excluded.resolution,
               last_seen_at  = excluded.last_seen_at",
            rusqlite::params![
                dir_name,
                res.path().map(|p| p.display().to_string()),
                res.tag(),
                now
            ],
        )?;
    }

    for s in &sessions {
        let res = &cache[&s.dir_name];
        // sessions.project_path is NOT NULL per PRD §6. When unresolved we
        // store the mangled directory name verbatim — obviously not a path,
        // so it can't be mistaken for a verified one. `projects.resolution`
        // remains the source of truth about whether a real path exists.
        let project_path = res
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| s.dir_name.clone());

        tx.execute(
            "INSERT INTO sessions
               (session_id, project_path, transcript_path, first_seen_at, last_active_at, preview)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(session_id) DO UPDATE SET
               project_path   = excluded.project_path,
               transcript_path= excluded.transcript_path,
               last_active_at = excluded.last_active_at,
               preview        = excluded.preview",
            rusqlite::params![
                s.session_id,
                project_path,
                s.transcript_path.display().to_string(),
                now,
                s.last_active_at,
                s.preview
            ],
        )?;
    }

    tx.commit()?;

    Ok(ScanSummary {
        projects: cache.len(),
        sessions: sessions.len(),
        unresolved: cache.values().filter(|r| r.path().is_none()).count(),
    })
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionView>> {
    let conn = state.db.lock().unwrap();
    // sessions.project_path holds the resolved path when there is one and the
    // mangled dir_name otherwise, which is exactly COALESCE on the project row.
    let mut stmt = conn.prepare(
        "SELECT s.session_id, p.dir_name, p.resolved_path, p.resolution,
                s.transcript_path, s.last_active_at, s.preview
           FROM sessions s
           JOIN projects p
             ON s.project_path = COALESCE(p.resolved_path, p.dir_name)
          ORDER BY s.last_active_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SessionView {
                session_id: r.get(0)?,
                dir_name: r.get(1)?,
                project_path: r.get(2)?,
                resolution: r.get(3)?,
                transcript_path: r.get(4)?,
                last_active_at: r.get(5)?,
                preview: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectView>> {
    let conn = state.db.lock().unwrap();
    let running: std::collections::HashSet<String> =
        tmux::list_sessions().unwrap_or_default().into_iter().collect();

    let mut stmt = conn.prepare(
        "SELECT p.dir_name, p.resolved_path, p.resolution,
                COUNT(s.session_id), COALESCE(MAX(s.last_active_at), '')
           FROM projects p
           LEFT JOIN sessions s
             ON s.project_path = COALESCE(p.resolved_path, p.dir_name)
          GROUP BY p.dir_name
          ORDER BY 5 DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let dir_name: String = r.get(0)?;
            let resolved: Option<String> = r.get(1)?;
            // Name from the resolved path when we have one, else the directory
            // name — the name must stay stable, so it keys off whichever
            // identifier is actually available.
            let name = tmux::session_name(resolved.as_deref().unwrap_or(&dir_name));
            Ok(ProjectView {
                tmux_running: running.contains(&name),
                tmux_session: name,
                dir_name,
                project_path: resolved,
                resolution: r.get(2)?,
                session_count: r.get::<_, i64>(3)? as usize,
                last_active_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Starts the project's detached tmux session if absent. Fails loudly for
/// unresolved projects rather than guessing a working directory.
#[tauri::command]
pub fn start_project_session(state: State<'_, AppState>, dir_name: String) -> Result<String> {
    let conn = state.db.lock().unwrap();
    let resolved: Option<String> = conn
        .query_row(
            "SELECT resolved_path FROM projects WHERE dir_name = ?1",
            [&dir_name],
            |r| r.get(0),
        )
        .unwrap_or(None);
    drop(conn);

    let path = resolved.ok_or_else(|| Error::UnresolvedProject(dir_name.clone()))?;
    let name = tmux::session_name(&path);
    tmux::ensure_session(&name, std::path::Path::new(&path))?;
    Ok(name)
}

#[tauri::command]
pub fn attach_command(name: String) -> String {
    tmux::attach_command(&name)
}

/// Clipboard text for PRD §5 item 2. Shelling out to the real binary is the
/// only supported resume path (CLAUDE.md rule 3).
#[tauri::command]
pub fn resume_command(session_id: String) -> String {
    format!("claude --resume {session_id}")
}

#[tauri::command]
pub fn tmux_available() -> bool {
    tmux::is_available()
}
