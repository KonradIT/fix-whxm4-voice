# AGENTS.md

## Project notes

`info.md` holds implementation notes for the Sony WH-1000XM4 voice-prompt tool.
Treat it as reference material: the format details, links to other implementations,
and suggested approaches. It is not a spec. It is written like a task, but its
decisions (CLI shape, subcommands, file layout, tooling, mapping format,
deliverables) are suggestions only, not requirements. Confirm a design decision
with the user, or record it in `docs/adr/`, before treating it as settled.

## Git

Never stage or commit unless explicitly told to. The user reviews unstaged
changes as a diff in another editor and stages them themselves. When told to
commit, stage everything first (unless told otherwise), then commit.

## Agent skills

### Issue tracker

Issues are tracked as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the five default triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), recorded as `Status:` lines. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
