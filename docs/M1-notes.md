# M1 — implementation notes

Tauri 2 scaffold, tmux-per-project persistence, Session Explorer.

## Running it

```
npm install
npm run tauri dev     # dev
npm run build         # frontend production build
cd src-tauri && cargo test
```

Database: `~/.local/share/cc-console/cc-console.db` (XDG). Delete it to force
a clean re-scan; migrations recreate it.

## Two encoder facts, discovered from real data

**1. The project-directory encoder collapses every non-alphanumeric character
to `-`, not just `/`.** Verified on real directories:

| Real path component | Encoded |
|---|---|
| `The Plan` (space) | `The-Plan` |
| `sleep_apnea_detection` (underscore) | `sleep-apnea-detection` |
| `cc-console` (literal hyphen) | `cc-console` |

The first implementation only treated `-` as separator-or-literal and
reported 3/13 projects resolvable. After correcting the model it resolves
10/13, and the remaining 3 are genuinely gone from disk.

Because the encoding is many-to-one it cannot be inverted. `project_path.rs`
therefore does not invert it — it searches: descend from `/`, read each real
directory, and keep entries whose *own* encoding matches the next run of
tokens. One readdir per level, and it handles every character class the
encoder collapses rather than a hardcoded list.

Outcomes are `Resolved` / `Ambiguous` / `Missing`. Unresolved projects stay
listed and their sessions stay searchable, but `project_path` is `null` to the
UI and launch actions are disabled. **Nothing ever fabricates a path.**

**2. The first transcript line is usually not a prompt.** Real files commonly
open with `queue-operation` bookkeeping records, so a literal first-line
preview renders JSON noise for a large share of sessions.

Preview uses approved strategy (c): scan the first 20 lines (capped at 256 KB),
and for each, walk the JSON value tree for the longest string that passes a
*structural* prose test — rejects UUIDs, ISO timestamps, whitespace-free
blobs, and strings under 8 chars. No field names (`type`, `message`,
`content`) are referenced anywhere, so an upstream schema change degrades the
preview to the raw first line instead of breaking it.

Result on real data: 42/42 sessions produced prose previews, 0 raw-JSON
fallbacks, 0 nulls.

## Security posture (CLAUDE.md rule 1)

`tmux::claude_argv()` returns exactly `["claude"]` — no permission-mode flags
at all, so sessions inherit Claude Code's own prompted default. Per-project
permission mode is an M4 settings-file concern and must never become a flag
here. `tmux::tests::never_passes_a_permission_bypass_flag` fails the build if
a forbidden flag ever appears, so this is enforced rather than intended.

`tmux has-session` uses the `-t=` exact-match form. Without it tmux matches
prefixes and would report a *different* project's session as the current
one — verified: `ccc-test` correctly does not match `ccc-test-verify`.

## Verified

- 24/24 Rust unit tests pass.
- `tsc --noEmit` clean; production frontend build clean.
- Real run against `~/.claude/projects`: 42 sessions, 10 projects
  (8 resolved, 2 missing), `user_version = 1`.
- Sidecar `subagents/*.jsonl` transcripts correctly excluded by the
  non-recursive glob.
- tmux round-trip: detached session created running bare `claude` (no flags),
  exact-match lookup, kill, cleanup.

## Not verified

**The GUI was not visually inspected.** This machine is Wayland; the
compositor denies programmatic screenshots, and the Tauri MCP driver needs a
bridge plugin the app doesn't carry. The backend was verified end-to-end
through the database the running app wrote, but the rendered window needs a
human eye.

## Deferred

- One preview picked up a system-reminder blob rather than the user's prompt,
  because "longest prose string on the first prose-bearing line" is a
  heuristic. Acceptable for cosmetic data; revisit only if it annoys.
