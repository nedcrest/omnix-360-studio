# How Omnix and Dailix were built with AI

Omnix (a home-video editor for 360 and ordinary footage) and Dailix (its
contact sheet: looking through a trip's footage and choosing it) were
built by one person, Dmytro Halushko, working with Claude in Claude Code.
He decided what to build and in what order, judged the design, and
tested every change on his own footage. Claude planned, wrote the code,
the tests and the docs, and audited its own work. Almost every commit
says so: `Co-Authored-By: Claude`.

This page is how that worked: the loop, the rules that kept the code
sound as it grew fast, and what each side did.

## In numbers

| | |
|---|---|
| Starting point | [rOsmo360](https://github.com/ravelund/rOsmo360) by Ole Morten Aaslund, 2026-10-22: a 360 player and simple editor, 19,000 lines of Rust, 48 tests |
| Omnix three weeks later (2026-11-12) | 50,000 lines of Rust, 400 tests (with 13 that export real files through ffmpeg), 940 commits over 18 days |
| Dailix (from 2026-11-04) | its app, 3,800 lines and its tests; its core, the contact sheet's (`crates/sheet`), now in Omnix's workspace |
| Roadmap | 113 cards done, 65 numbered gaps closed, each tested by the user before it was called done |

## The loop

1. **An idea becomes a numbered gap.** Anything the user says he wants
   («the music can't be lowered under speech», «I can't see which edit
   this is») gets a number and a section in `docs/omnix/EDITOR-GAPS.md`,
   and a card on the roadmap, with a milestone. Ideas come unsorted;
   sorting them is part of the job.
2. **Bigger work is planned on paper first.** A design page (for example
   `docs/omnix/OPS-API.md`, `docs/omnix/TIME-SEAM.md`) says why, what
   changes, what could go wrong and in what steps. The user reads it,
   decides the open questions, and only then is code written.
3. **Small local commits.** One subtask, one commit, its message saying
   why and what was measured. Nothing is pushed until the user says so.
4. **Tests, then the user's eyes.** Every change builds without warnings
   and passes all tests. Then its card goes to «waiting for a check»,
   and the user tries it on real footage: he tests the main paths and
   reports what he sees, often with a screenshot.
5. **Done means done.** Only after his check does a card move to «Done».
   A bug he finds later is a new case, not a check that failed.

The user follows all of this on a live page: `docs/omnix/roadmap/` serves
`ROADMAP.md` as cards, redrawn the moment the file changes.

## Rules that kept it sound

Fast work goes wrong in predictable ways. The ways it went wrong here
were written down as rules the agent reads before touching the code:
`.claude/skills/code-rules/SKILL.md` (both repos), `AGENTS.md`.

- **A change to a project is an operation.** Every edit (trim, move,
  fade, a key, a voice) is an op in `src/model/ops/`, checked, undoable,
  with a test. The window only builds ops. The same ops are open to the
  command line and to an AI (`omnix apply`, `omnix describe`,
  `docs/omnix/OPS-API.md`), so a script or a model can edit a film the way
  the user does.
- **Rules are pure functions with tests**, never logic hidden in a click
  handler, where only clicking could test it.
- **One fact, one place.** The second copy of anything is the moment to
  share it.
- **Functions stay short.** Clippy's `too_many_lines` at 80 lines; a
  function that is touched and too long is split first.
- **The CPU and the GPU agree.** The player draws on the GPU, the export
  can use either. Tests render both ways and compare.
- **Done means done**: a checklist before any card waits for the user (an
  op with a test, `describe` says the new value, no new warning, no long
  function).

## Habits of the agent, made into tools

- **The clock.** A hook gives Claude the local time with every message,
  so «you've been at it for days» or «it's 4 a.m.» is a fact, not a
  guess.
- **Audits by measure.** A hook counts the code added since the last
  audit and asks for a small one after about 2,000 lines or 40 commits.
  Both hooks, and how they're wired: [`docs/ai/hooks/`](ai/hooks/).
  One or two review agents read what changed; findings are sorted (bugs
  now, the rest where it belongs) into `docs/omnix/AUDIT.md`.
- **Memory across sessions.** Long work outlives one conversation.
  `docs/omnix/CONTEXT.md` holds a return point, rewritten (not appended)
  at every pause, so a new session starts where the last one stopped.
- **Safety nets before a refactor.** Before the time seam (putting every
  clip-time ↔ edit-time conversion behind a few functions, for playback
  speed), the agent wrote snapshots of every time the code derives
  (`src/time_snapshot.rs`, `tests/golden/`) and a test that a cut at
  twice the speed must sit on the edit as one half as long. That test
  found two wrong clocks on its first run.

## Who did what

**The user** chose the product: an editor for his family's travel
footage, on Linux, with sound treated as carefully as the picture. He set
every priority, named the problems from using it («the scooter should
stay framed at second five»), judged each design, and caught what no test
sees: a drag that felt wrong, a colour that didn't read, a song that
drifted from its picture. He kept asking whether it was still the editor
he wanted («it feels like we're making another Kdenlive»), and held the
queue closed: a new idea waits in «later» until the current list is done.

**Claude** turned that into plans, code, tests and docs: the operations
core, the sound lanes and mixing, sync lock, groups, framing with
keyframes, the contact sheet, the audits. It kept the record (gaps,
roadmap, context) so nothing was lost between sessions, and it was held
to the rules above when it wanted to cut corners.

## Where to look

- `docs/omnix/ROADMAP.md` and `EDITOR-GAPS.md` (with `-DONE`): every idea
  and what became of it (the roadmap is in Ukrainian, the user's
  language; the gaps are in English).
- `docs/omnix/OPS-API.md`, `CORE-API.md`, `TIME-SEAM.md`: design pages
  written before the code.
- `docs/omnix/AUDIT.md`: the latest small audits, what they found and
  what was fixed.
- `git log`: every step, with its reason.
