# PRD — Claude Code Session & Skill Console

**Working name:** `cc-console` (placeholder — rename freely)
**Platform:** Linux (Fedora primary target), Tauri 2 desktop app
**Author:** Nithish Kannan M
**Status:** Draft v1

## 1. Problem

Claude Desktop, which provides a GUI for session browsing and skill management,
is not available on Fedora/RHEL (Debian-based distros only as of this writing).
CLI-only workflows leave three gaps:

1. **No visual session history.** `claude --resume` is a terminal picker,
   scoped per-project by default. There's no cross-project, cross-time view
   of past work.
2. **No durable findings layer.** Session summaries and key decisions live
   inside JSONL transcripts (internal format, not stable across Claude Code
   versions) or are lost when a session ends. `--continue`/`--resume` context
   restoration is also documented as occasionally unreliable
   (anthropics/claude-code#43696), so relying on it alone is fragile.
3. **No skill/plugin intelligence.** Skills are directories with `SKILL.md`
   files, discoverable but not browsable, comparable, or bloat-checked.
   Published research indicates most installed skills reduce output quality
   rather than improve it — the real need is bloat/gap detection, not just
   a listing.

## 2. Goals

- Give a native GUI for browsing, resuming, and searching Claude Code
  sessions across all projects on the machine.
- Maintain a durable, queryable findings index independent of whether
  native session resume succeeds.
- Provide a skill/plugin browser with bloat and coverage-gap detection,
  not just an enumeration.
- Enforce a real permission-mode security model — never a global
  "skip all checks" toggle.

## 3. Non-goals

- Do not reimplement `--continue`/`--resume` conversation restoration.
  Shell out to the real `claude` binary for that.
- Do not parse the internal JSONL transcript schema for anything beyond
  filename, mtime, and first-line preview. Anything requiring transcript
  content goes through `claude -p --resume <id> --output-format json`.
- Do not build a multi-user product. Single local user, single machine.
- Do not ship a global permission-bypass setting (see §7, Security).

## 4. Orchestration strategy

This app is a GUI and intelligence layer over existing plumbing, not a
from-scratch session engine.

| Concern | Approach |
|---|---|
| Session persistence across terminal restarts | Reimplement the tmux-per-project pattern popularized by `claunch` natively in Rust (do not vendor claunch's shell script — it defaults to `--dangerously-skip-permissions`, which this project does not adopt). One tmux session per project, named deterministically from the project path. |
| Findings capture / semantic search | Evaluate `claude-session-continuity-mcp` as the extraction/storage engine before building a custom hook pipeline. If its on-disk format is directly queryable (confirm during Milestone 2), read from it. Otherwise fall back to the custom `SessionEnd` hook + SQLite design in §6. |
| Session resume | Shell out to `claude --resume <id>` / `claude --continue`. Never reimplement. |
| Skill enumeration | Direct filesystem read of `~/.claude/skills/`, project `.claude/skills/`, and plugin skill directories. Parse `SKILL.md` YAML frontmatter per the documented schema. |

## 4.1 Prior art survey

Before implementing any MVP feature, check this table. Read source, don't
just skim READMEs — several of these are close enough to fork or lift
patterns from directly.

| Tool | What it is | Verdict |
|---|---|---|
| `tombelieber/claude-view` | Rust backend, SIMD-accelerated JSONL parsing, mmap I/O, SQLite session index, live multi-session dashboard, cost tracking, search, Claude Code plugin exposing 85 MCP tools back into sessions | **Read source before writing the Session Explorer scanner.** Closest architectural match to MVP items 1 and 5. Has at least one real bug history (rustls crypto-provider panic on macOS arm64, fixed). Official builds send anonymous usage analytics by default (opt out via env var or build from source) — check its license and decide fork vs. reference-only before M1. |
| `Dicklesworthstone/coding_agent_session_search` (cass) | Rust, indexes 23 coding agents into a normalized SQLite schema via per-agent connectors (`Box<dyn Connector>`, parallel via `rayon`), resilient to malformed/legacy entries and culled transcripts | Reference for parser resilience — model the JSONL scanner's error handling on this rather than assuming well-formed input every time. |
| `delexw/claude-code-trace` | Rust+Node, desktop/web/TUI viewer, live-tail sessions in progress | Reference for live-tail UX if sessions should update in the explorer while still running, not only after they end (v2 candidate). |
| `henchmarketing-rgb/headroom` | Small statusline tool: reads most-recently-modified session JSONL per project, extracts last token-usage entry, shows context-fill % | Adopt the pattern directly — same file-scanning logic the Session Explorer already needs. Feeds v2 item 12 below. |
| `thedotmack/claude-mem` | Full memory system: background worker daemon, AI-compressed observations, SQLite, web dashboard, MCP search tools | Reference architecture only, not a dependency. High volume of install/uninstall and daemon-reliability issues (worker crashes, stuck dashboards, a reported speed regression while running). Don't take a runtime dependency on this for something as central as the findings layer. |
| `rebelytics/one-skill-to-rule-them-all` (task-observer) | Not a running tool — a Claude Code skill. Logs correction patterns, repeated workflows, and effective techniques from real sessions to a filesystem observation log, feeding scheduled skill-improvement reviews | **Adopt directly.** Zero runtime cost (markdown skill, no daemon), CC BY 4.0. Install into `.claude/skills/task-observer/` now, independent of the app build. Its observation log (`skill-observations/`, `skill-updates/`) is a real signal source for v2 item 7 (skill bloat/gap detection) — stronger than static file-type heuristics alone. |

## 5. Features

### MVP (Milestone 1–2)

1. **Session Explorer** — list sessions across all projects by scanning
   `~/.claude/projects/*/*.jsonl` for filename (session ID), directory
   (project), mtime, and first-line preview only.
2. **Resume shortcut** — button that either copies
   `claude --resume <id>` to clipboard or launches a terminal at the
   project directory with the command pre-filled.
3. **On-demand summarize** — runs
   `claude -p --resume <id> --output-format json "summarize: decisions, files touched, open questions"`,
   caches result in SQLite keyed by session ID + transcript last-line hash.
4. **Findings capture** — prompted extraction button (not hook-based for
   MVP): asks Claude to list findings worth remembering from a given
   session, stores results in SQLite.
5. **Cross-session search** — SQLite FTS5 over cached summaries + findings.
6. **Skill browser (basic)** — list skills from all three locations with
   name, description, scope, enabled state.

### v2 (Milestone 3+)

7. **Skill bloat / gap detection** — flag skills unused in N sessions;
   flag files/languages worked on with no matching skill coverage.
8. **Per-file semantic skill relevance** — surface a skill only when a
   currently-open file's content matches its description above a
   similarity threshold.
9. **Multi-project dashboard** — single view of "which terminal/tmux
   session has which project," addressing the most repeated pain point
   in prior art.
10. **`SessionEnd` hook-based findings capture** — automatic extraction
    once the prompted-extraction UX (item 4) has validated what's worth
    capturing.
11. **Skill toggling from the GUI** — write to `skillOverrides` in
    `.claude/settings.local.json` (`"on"` / `"name-only"` /
    `"user-invocable-only"` / `"off"`).
12. **Context-fill indicator per session** — surface context-window %
    in the Session Explorer, sourced the same way `headroom`
    (henchmarketing-rgb) does: most-recently-modified session JSONL,
    last token-usage entry. Directly backstops the findings index's
    reason for existing — compaction can silently drop work in
    progress mid-session.
13. **Skill-gap signal from task-observer's observation log** — feed
    `skill-observations/` entries into the bloat/gap detection in item
    7 as a stronger real-world signal than static file-type matching
    alone.

## 6. Data model (SQLite)

```sql
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
```

## 7. Security requirements (non-negotiable)

- No app-wide toggle that maps to `--dangerously-skip-permissions` or
  `bypassPermissions`. Every permission-mode change is scoped to a single
  project and written to that project's `.claude/settings.local.json`.
- Default permission mode for any project the app touches: standard
  (prompted). `bypassPermissions` is opt-in, per project, every time —
  matching Claude Code's own behavior of never restoring
  `bypassPermissions` on resume.
- Any session running in `bypassPermissions` mode shows a persistent,
  always-visible UI indicator for the session's full duration — not just
  a one-time confirmation dialog.
- Every bypass-mode session is logged to `bypass_sessions` (project,
  mode, start/end time) regardless of outcome.
- The GUI should default-recommend `acceptEdits` or `dontAsk` over
  `bypassPermissions` wherever the UI presents mode choices, and should
  make `bypassPermissions` visually and interactionally the "deliberately
  inconvenient" option, not a peer choice.

## 8. Tech stack

- **Backend:** Rust, Tauri 2
  - `notify` crate for filesystem watching (`~/.claude/skills/`,
    `.claude/skills/`, `~/.claude/projects/`)
  - `rusqlite` for local storage
  - Shell out to `claude` and `tmux` via `std::process::Command`
- **Frontend:** React + TypeScript (or plain Tauri + vanilla — defer to
  whatever Claude Code scaffolds by default for a Tauri 2 + React
  template)
- **Target OS:** Fedora (test on current Fedora release), should remain
  portable to other Linux distros since nothing here is Fedora-specific
  beyond the absence of Claude Desktop

## 9. Milestones

| Milestone | Deliverable |
|---|---|
| M0 | Install `task-observer` skill locally (§4.1); read `claude-view` and `cass` source before writing any scanner code |
| M1 | Tauri app scaffold; native tmux-per-project session persistence (claunch-equivalent, no bypass flag); Session Explorer UI reading transcript metadata |
| M2 | On-demand summarize + prompted findings capture wired to SQLite; cross-session FTS search; evaluate claude-session-continuity-mcp integration |
| M3 | Skill browser (basic enumeration) |
| M4 | Security model: per-project permission mode UI, bypass session logging and persistent indicator |
| M5 | Skill bloat/gap detection, multi-project dashboard |

M6 and its full spec (MCP server exposing findings/skills back into
live sessions, companion skill) live in `MCP_AND_SKILLS.md` — build
after M2 and M3 are populated with real data.

## 10. Open questions (flag these back before assuming an answer)

- Confirm claude-session-continuity-mcp's on-disk storage format before
  committing to read it directly (npm package internals may change).
- Decide GUI framework (React vs Svelte vs vanilla) based on what the
  Tauri 2 scaffold defaults to, unless there's a stated preference.
- Decide whether tmux is a hard dependency or the app should also support
  a tmux-less "direct" mode (claunch offers both).
- Decide fork-vs-reference-only for `claude-view` after reading its
  license and confirming its Linux/Fedora support is solid — if strong,
  this could shortcut most of M1's scanner and dashboard work.
