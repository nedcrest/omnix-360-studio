# Omnix: for Claude Code

Omnix and its sibling Dailix are built with Claude Code
([`docs/AI-DRIVEN.md`](docs/AI-DRIVEN.md) says how). Start here:

- **[`AGENTS.md`](AGENTS.md)**: what Omnix is, how to build and check it,
  the CPU/GPU pairs that must stay alike, the layout of the code.
- **The skills** in [`.claude/skills/`](.claude/skills/):
  [`code-rules`](.claude/skills/code-rules/SKILL.md) before writing or
  changing any Rust (and the checklist before a commit);
  [`roadmap`](.claude/skills/roadmap/SKILL.md) whenever work starts,
  finishes or waits for a check.
- **`docs/omnix/`**: the roadmap (`ROADMAP.md`, in Ukrainian, the user's
  language), every idea by number (`EDITOR-GAPS.md`, done ones in
  `EDITOR-GAPS-DONE.md`), the design pages written before the code
  (`OPS-API.md`, `CORE-API.md`, `TIME-SEAM.md`, `PROJECT-CARDS.md`), the
  latest audits (`AUDIT.md`), and `CONTEXT.md`, where long work picks up
  after a pause.

The essentials, in short: a change to a project is an operation
(`src/model/ops/`) with its range check and a test, so the command line
and an agent can make it too; a rule is a pure function with a test, the
window only calls it; one fact lives in one place; functions stay under
80 lines; nothing is committed that doesn't build without warnings and
pass its tests; commits are small and local until the user says push.

rOsmo360's maintainer notes, which this file held until 2026-11-14 (its
author's paths and footage), are in the git history.
