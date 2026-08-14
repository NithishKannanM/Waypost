# MCP Server & Skills — cc-console

Companion spec to `PRD.md`. Covers the one thing the PRD's core app
doesn't do yet: closing the loop so a *live* Claude Code session can
query the findings/skills data cc-console has already indexed, instead
of that data being GUI-only. This is the same pattern `claude-view` and
`claude-mem` both use (MCP tools + companion skills exposed back into
Claude Code) — but scoped to only what this project's SQLite schema
(PRD §6) actually holds.

Build this after PRD Milestone 2 (findings capture + SQLite are
populated) and Milestone 3 (skill index exists). No point exposing an
MCP server over empty tables.

## 1. Why an MCP server, not just the GUI

The GUI (Tauri app) is for browsing after the fact. The MCP server is
for *during* a session — so you can ask Claude Code directly "have we
solved this before" or "what skills apply here" without alt-tabbing to
the app. Same local SQLite database, two access paths.

## 2. Server: `cc-console-mcp`

- **Transport:** local stdio (no network exposure — matches the PRD's
  local-first, single-user stance)
- **Storage:** reads the same SQLite file the Tauri app writes to
  (PRD §6 schema) — read-mostly, with a narrow write path for
  `save_finding` only
- **Registration:** `claude mcp add cc-console-mcp -- <binary path>`,
  or ship as a Claude Code plugin (marketplace entry) once stable, the
  way claude-view distributes its plugin

### Tools to expose

| Tool | Input | Output | Notes |
|---|---|---|---|
| `search_findings` | `query: string`, `limit: int` (default 10) | matching rows from `findings_fts` with `session_id`, `project_path`, `finding`, `captured_at` | Primary "have I solved this before" tool |
| `get_session_summary` | `session_id: string` | cached row from `summaries`, or a note that none exists yet | Read-only, never triggers a new summarization call from inside the MCP tool — that stays an explicit GUI action (PRD §5 item 3) to avoid surprise API/subscription usage mid-session |
| `save_finding` | `finding: string`, `session_id: string`, `project_path: string` | inserted row id | The only write path. Source is always `'mcp'` — add this as a third allowed value alongside `'prompted'` / `'hook'` in the `findings.source` CHECK constraint (PRD §6) |
| `list_skills` | `scope: 'personal' \| 'project' \| 'plugin' \| 'all'` (default `all`) | rows from `skills` table | Straight read of what M3's skill browser already indexes |
| `get_skill_recommendations` | `context: string` (e.g. current file's language/content signature) | ranked skill matches + any bloat/gap flags from PRD §5 items 7 and 13 | Depends on the semantic relevance work in PRD §5 item 8 — stub this to keyword matching until that's built, don't block the tool on it |

### Explicitly not exposed

- No tool that changes permission mode or touches `bypass_sessions` —
  that stays GUI-only, per PRD §7. An MCP tool a live session could
  call to grant itself `bypassPermissions` would defeat the entire
  security model.
- No tool that triggers `--resume`/`--continue` on another session —
  session control stays a human action in the GUI, not something one
  live session can do to another.

## 3. Skills

### Adopt, don't build

`task-observer` (`rebelytics/one-skill-to-rule-them-all`) — already
covered in PRD §4.1. Install as-is at `.claude/skills/task-observer/`.
Its observation log feeds `get_skill_recommendations` above once M3's
gap-detection logic reads from it.

### Build: `cc-console-usage` (companion skill, not an app feature)

A thin skill whose only job is teaching Claude *when* to reach for the
MCP tools above, since tool descriptions alone are often missed under
task focus (the same problem task-observer's own docs flag about
description-matching). Keep it short — this is a trigger-condition
skill, not a knowledge base.

Suggested `SKILL.md` frontmatter and trigger conditions:

```yaml
---
name: cc-console-usage
description: >
  Use when starting work that resembles something done before in this
  project or others (a bug, a config problem, a "have I done this"
  moment) — call search_findings before re-deriving an answer from
  scratch. Use when a session produces a decision, a gotcha, or a fix
  worth remembering — call save_finding before the session ends. Use
  when picking a skill for the current file/task — call
  get_skill_recommendations instead of browsing .claude/skills/
  manually.
---
```

Body content: two or three short trigger examples per tool (mirroring
how task-observer's own docs stay pattern-level rather than
exhaustive), plus the one hard rule: **never call `save_finding` with
anything from `bypass_sessions` context or anything the user marked
sensitive** — findings are meant for technical decisions, not incident
detail dumps.

## 4. Hooks needed to keep the MCP data fresh

| Hook | Trigger | Action |
|---|---|---|
| `SessionEnd` | session closes | Write session metadata (id, project, mtime) into `sessions` table if not already present — keeps the MCP server's view current even for sessions the GUI hasn't scanned yet |
| `PostToolUse` (optional, v2) | after Write/Edit calls | Update `skills` table's `last_seen_at` if the touched file matches a skill's relevance signature — feeds bloat detection without a separate scan pass |

Do not add a `SessionStart` hook that auto-injects findings into every
new session's context. That's the claude-mem pattern (§4.1) and it's
exactly the kind of always-on behavior tied to that project's
performance complaints. Keep retrieval pull-based (`search_findings`
called on demand) not push-based.

## 5. Milestone placement

Add as **M6**, after PRD §9's M5:

| Milestone | Deliverable |
|---|---|
| M6 | `cc-console-mcp` server (search_findings, get_session_summary, save_finding, list_skills, get_skill_recommendations); `cc-console-usage` skill; `SessionEnd` hook for session-table freshness |
