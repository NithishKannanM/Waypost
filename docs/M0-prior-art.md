# M0 — Prior-art findings & decisions

Deliverable for PRD §9 M0. Read before starting M1's scanner or the
SQLite index. Records the answer to PRD §10 open question 4.

## task-observer — INSTALLED

- Source: `rebelytics/one-skill-to-rule-them-all`
- License: **CC BY 4.0** (confirmed — `LICENSE.txt` is the verbatim
  Attribution 4.0 International text). PRD §4.1's claim holds.
- Installed to `.claude/skills/task-observer/` (SKILL.md, USER-GUIDE.md,
  LICENSE.txt, references/). Copied verbatim; the attribution block in
  SKILL.md satisfies CC BY 4.0's credit requirement.

### Observation log path — matters for M5 item 13

The log lives at `[workspace folder]/skill-observations/log.md`, where
the skill defines `[workspace folder]` for Claude Code as the *stable
project identity* — `~/.claude/projects/<project-id>/` — explicitly NOT
cwd (a worktree/ephemeral checkout takes the log with it when torn down).

Two consequences for M5's gap detection:

1. The log is **prose markdown**, not structured data. M5 item 13 will be
   parsing free text, not reading records. Budget for that.
2. Related paths that also exist: `skill-observations/last-review-date.txt`,
   `skill-observations/archive/log-[YYYY-MM-DD].md`,
   `skill-updates/[date]/[skill-name]/`.

### Open item — activation is not automatic

The skill's own description states that description-level matching alone
"is not enforceable" and recommends pairing it with a CLAUDE.md
instruction or a session-start hook. **Not wired up.** Deferred to the
user — note it interacts with CLAUDE.md rule 6 (no push-based
SessionStart injection), so wiring it needs a deliberate decision, not a
default.

## claude-view (`tombelieber/claude-view`) — REFERENCE ONLY

License is **MIT** (Copyright 2025-2026 Tom Tang) — a fork would be
legally clean. Recommendation is reference-only anyway, for three
independent reasons:

**1. It is not a desktop app.** Zero Tauri dependencies anywhere in
`Cargo.lock`. It is an `axum` web server + React web frontend, plus
`apps/mobile`, `apps/landing`, `apps/share`, a `sidecar/` Bun service,
`supabase/`, and `crates/relay`, `crates/server-teams`. ~197k lines of
Rust across 21 crates. cc-console is a single-user local Tauri 2 app;
almost none of that structure transfers.

**2. Linux support is "Planned", not shipped.** README's platform table
lists `Linux (x64) | Planned`; prebuilt binaries are macOS-only
(`install.sh` accepts linux as a target triple but no release artifact
exists). PRD §10 item 4 conditioned the fork decision on Linux/Fedora
support being *solid*. It is not. This alone settles the question.

**3. Its core competency is the thing CLAUDE.md rule 2 forbids.** Its
value is deep `SessionStats` extraction from the JSONL transcript
schema. We are barred from parsing that schema beyond filename, mtime,
and first line.

### Confirmed cost of schema-parsing — evidence for rule 2

`crates/session-parser/src/version.rs` documents its own breakage
history in `STATS_VERSION` (now v4):

- v3 — "Migration 88 adds these columns; indexer will re-extract rows
  with stats_version < 3"
- v4 — "42 additions in migration 89 ... before the IRREVERSIBLE DROP
  in 7.h.6"

Four stats-version bumps and ~89 migrations to track an upstream format
Anthropic documents as unstable. This is empirical support for rule 2,
not just a stylistic preference.

### Two patterns worth lifting (MIT, attribute if copied)

**`crates/session-parser/src/staleness.rs` — blake3 head+tail hashing.**
Hashes the first and last 64 KB of a file into one digest (plus a
line-aligned mid-region hash as a sibling). Detects appends and mid-file
edits without reading multi-MB files. This is **rule-2 compliant** —
hashing raw bytes is not parsing the schema.

Relevant to PRD §5 item 3, which specifies caching summaries keyed by
"session ID + transcript last-line hash". Head+tail is cheaper and
strictly more edit-sensitive than a last-line hash. **Flagged as an M2
decision, not decided here** — the PRD named last-line hash explicitly.

**Its error handling is a counter-example, not a model.** `parse_jsonl`
returns `Err` on the *first* malformed line; the doc comment admits
callers wanting "drop-and-continue semantics must wrap this and discard
the `Err`". Do not copy this. Our scanner should skip bad lines, not
abort a file.

### Telemetry

Official builds send anonymous usage analytics **on by default**; opt out
with `CLAUDE_VIEW_TELEMETRY=0`; source builds send nothing. Confirms PRD
§4.1. Not a concern under reference-only.

## cass (`Dicklesworthstone/coding_agent_session_search`) — BLOCKED, NOT USED

**Do not lift code from this repo without an explicit decision from the
project owner.**

Its `LICENSE` is not plain MIT. It is "MIT License (with OpenAI/Anthropic
Rider)". The rider:

- Defines "Restricted Parties" as OpenAI, **Anthropic PBC**, their
  affiliates, and anyone "acting directly or indirectly on behalf of, for
  the benefit of, or under the direction of" them.
- Grants **no rights whatsoever** to any Restricted Party, and voids any
  purported sublicense to one.
- Defines "use" to include "benchmarking, testing, **analyzing,
  indexing**, or incorporating the Software or any Derivative Works into
  any dataset, training corpus, evaluation harness, or **pipeline for
  machine learning or other automated systems**."
- Terminates all permissions automatically on breach, and requires the
  rider to be reproduced unmodified in any distribution of a derivative
  work.

Having an Anthropic model read and analyze this source in order to derive
patterns for another codebase sits squarely in what that rider targets.
Source analysis was **stopped at the license file**; the connector and
parser internals were not read, and nothing from cass informs the M1
design.

### This costs us approximately nothing

PRD §4.1 wanted cass only as a "reference for parser resilience". Under
CLAUDE.md rule 2 the scanner parses filename, mtime, and one line — there
is no multi-agent connector abstraction to model and very little parsing
to make resilient. The needed behaviour is one rule: *skip anything
unreadable, never abort the scan*. That does not require a reference
implementation.

**Recommendation:** drop cass from the prior-art dependency list and
implement scanner resilience directly. Escalate only if the owner wants
the rider assessed by someone qualified to do so.

## Net effect on M1

- Write the Session Explorer scanner from scratch. No fork.
- Scanner contract: enumerate `~/.claude/projects/*/*.jsonl`; take
  session ID from filename, project from directory, `last_active_at`
  from mtime, preview from the first line only. Never abort a scan on a
  malformed file — skip and continue.
- SQLite index follows PRD §6 as written, via an explicit migration file
  (CLAUDE.md tech-stack rule). No schema borrowed from prior art.
- blake3 head+tail staleness hashing is a candidate for M2's summary
  cache key. Decide at M2.

## Still open, by milestone

- **M1** — PRD §10 item 2 (GUI framework) and item 3 (tmux hard
  dependency vs. tmux-less direct mode).
- **M2** — PRD §10 item 1 (claude-session-continuity-mcp on-disk format);
  plus the head+tail vs. last-line hash choice above.
