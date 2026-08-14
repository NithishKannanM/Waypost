-- Full PRD §6 schema. §6 is the source of truth; later milestones use these
-- tables rather than each shipping their own CREATE. M6's findings.source
-- CHECK change ('mcp') lands as 002, not an edit to this file.

CREATE TABLE sessions (
  session_id TEXT PRIMARY KEY,
  project_path TEXT NOT NULL,
  transcript_path TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_active_at TEXT NOT NULL,
  preview TEXT
);

CREATE TABLE summaries (
  session_id TEXT NOT NULL REFERENCES sessions(session_id),
  transcript_hash TEXT NOT NULL,
  summary TEXT NOT NULL,
  generated_at TEXT NOT NULL,
  PRIMARY KEY (session_id, transcript_hash)
);

CREATE TABLE findings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL REFERENCES sessions(session_id),
  project_path TEXT NOT NULL,
  finding TEXT NOT NULL,
  captured_at TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('prompted', 'hook'))
);

CREATE VIRTUAL TABLE findings_fts USING fts5(
  finding, content='findings', content_rowid='id'
);

CREATE TABLE skills (
  path TEXT PRIMARY KEY,
  name TEXT,
  description TEXT,
  scope TEXT CHECK (scope IN ('personal', 'project', 'plugin')),
  last_seen_at TEXT NOT NULL
);

CREATE TABLE bypass_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_path TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT
);

-- Beyond §6: the scanner's unit of iteration is the mangled directory name
-- under ~/.claude/projects/. That name is lossy (path separators and literal
-- hyphens both encode as '-'), so the real cwd may be unrecoverable. Cache
-- resolution results here so the filesystem probe runs once per project,
-- and so unresolved projects stay listable without a usable path.
CREATE TABLE projects (
  dir_name TEXT PRIMARY KEY,
  resolved_path TEXT,
  resolution TEXT NOT NULL CHECK (resolution IN ('resolved', 'ambiguous', 'missing')),
  last_seen_at TEXT NOT NULL
);

CREATE INDEX idx_sessions_project ON sessions(project_path);
CREATE INDEX idx_sessions_last_active ON sessions(last_active_at DESC);
CREATE INDEX idx_findings_session ON findings(session_id);
