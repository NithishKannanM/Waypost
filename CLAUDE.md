# CLAUDE.md — cc-console

This file is auto-loaded by Claude Code at session start. Read `PRD.md`
and `MCP_AND_SKILLS.md` in full before writing any code.

## What this project is

A Tauri 2 desktop app (Rust backend, Fedora-first, portable to other Linux)
that gives a GUI for Claude Code session browsing, cross-session findings
search, and skill/plugin intelligence — plus an MCP server (`cc-console-mcp`,
built in Milestone 6) that exposes the same findings/skills data back into
live Claude Code sessions. It orchestrates existing tools (tmux-based
session persistence pattern, `claude` CLI) rather than reimplementing
Claude Code's own session engine. See `PRD.md` §4.1 for the full prior-art
survey — several existing tools (claude-view, cass, headroom, task-observer)
are either reference architecture or direct dependencies; read that section
before writing a scanner, parser, or skill from scratch.

## Non-negotiable rules

1. **Never default to `--dangerously-skip-permissions` or
   `bypassPermissions`.** Every permission-mode change is per-project,
   opt-in, logged, and shown with a persistent UI indicator while active.
   See PRD §7. If a feature request would require a global bypass toggle,
   stop and flag it instead of implementing it.
2. **Do not parse Claude Code's internal JSONL transcript schema** beyond
   filename (session ID), file mtime, and the first line for a preview.
   Anthropic documents this format as internal and version-unstable. Any
   feature needing real transcript content goes through
   `claude -p --resume <id> --output-format json` or `/export`.
3. **Do not reimplement `--continue`/`--resume` conversation restoration.**
   Shell out to the real `claude` binary. This project's job is
   visualization and a durable findings index on top of it, not a
   replacement for it.
4. **claunch is reference material, not a dependency.** Read its approach
   (tmux session per project, deterministic naming) but reimplement the
   relevant logic natively in Rust without its default
   `--dangerously-skip-permissions` behavior.
5. **The MCP server (Milestone 6) never exposes permission-mode control or
   session-control tools.** No tool that changes `bypassPermissions`, writes
   to `bypass_sessions`, or triggers `--resume`/`--continue` on another
   session. See `MCP_AND_SKILLS.md` §2, "Explicitly not exposed" — this is
   a hard boundary. A live session calling into the security layer would
   defeat rule 1 entirely. If a tool under design starts needing either
   capability, stop and flag it instead of building it.
6. **Retrieval into live sessions is pull-based, not push-based.** No
   `SessionStart` hook that auto-injects findings into every new session's
   context — that's the claude-mem pattern (PRD §4.1) tied to its own
   reported performance complaints. `search_findings` is called on demand
   by the model or the user, never fired automatically at session start.

## Tech stack

- Rust + Tauri 2, `rusqlite`, `notify`
- Frontend: whatever the Tauri 2 + React/TS scaffold provides by default
  unless told otherwise
- SQLite schema is defined in `PRD.md` §6 — treat it as the source of
  truth, migrate with explicit migration files rather than ad hoc `ALTER
  TABLE` calls scattered through the codebase. The `findings.source` CHECK
  constraint gains a third value (`'mcp'`) in Milestone 6 — that's also a
  migration, not a schema rewrite.
- Milestone 6 adds an MCP server (local stdio transport, read-mostly
  against the same SQLite file) and one companion skill
  (`cc-console-usage`) — see `MCP_AND_SKILLS.md` in full before touching
  either.

## Working style

- Build in milestone order: `PRD.md` §9 (M0–M5), then `MCP_AND_SKILLS.md`
  §5 (M6). Don't jump ahead — M6 specifically depends on M2 and M3 having
  real data in SQLite, not empty tables, and v2 features (skill relevance,
  bloat detection) shouldn't start before M1–M2 are solid.
- If following the master build prompt: treat each milestone as a
  checkpoint — plan, wait for explicit go-ahead, build, summarize, commit
  to git before moving on. Don't collapse milestones together even if the
  next one seems like a natural continuation.
- Prefer plan mode for anything touching the security model (§7) or the
  MCP server's tool surface (rule 5 above) — walk through the design
  before writing permission-handling or tool-definition code.
- When something in the PRD is ambiguous or you're about to guess at an
  API/format detail (e.g. claude-session-continuity-mcp's storage
  layout, or whether claude-view is fork-viable), stop and ask rather
  than assuming — see PRD §10 for known open questions.
- If context is lost mid-build (compaction, a crashed terminal, a fresh
  session), check actual repo state before assuming which milestone is
  current — don't guess from memory of the conversation.
- Write direct, engineering-first commit messages and code comments.
  Skip filler explanation of what standard code does; comment on why,
  not what.

