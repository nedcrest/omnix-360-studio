---
name: code-rules
description: The rules for changing Omnix's code, and the checks before a commit. Use before writing or changing any Rust in this repo, when reviewing it, and when the user asks for a refactor or a cleanup.
---

# Changing Omnix's code

`AGENTS.md` is the long form (the CPU/GPU pairs, the facts not to re-derive,
the layout). This page is what to hold in mind while writing, learned from
building fast and from the audit of 2026-11-05 (in git; what was kept: `ARCHITECTURE.md`): the
cost was almost never dead code; it was the same fact written in several
places, drifting apart, and rules living in UI handlers without tests.

## Before writing

- **Look for the place it already lives.** Before a helper, a constant, a
  list or a formula: `grep` for it in this repo and in `../dailix`. One
  place for each fact (a clock, a role's colour, file types, a cut's media,
  the app's folders, a JPEG frame). Shared by both programs: in
  `dailix_sheet` (sheet facts) or in this library (media, folders).
- **A change to a project is an op.** Add it to `src/model/ops/` with its
  range check and a test; the UI builds the op (inside `edit.begin()` /
  `commit()` for a slider or a drag). Never write `self.edit.<field> = …`
  from a handler: the CLI and an agent couldn't do it, and nothing checks it.
- **A rule goes in a module with tests; the window only carries it out.**
  What a key does, what to leave out when a file is gone, what a sheet's
  marking makes: pure functions where they can be (see
  `dailix_sheet::marking`, `Project::drop_missing`).
- **A `pub` item is checked in both repos** before it's called unused:
  Dailix links this library (`viewer`, `from_sheet`, `app_dir`, ...).

## While writing

- CPU and GPU paths match, with a test that renders both (AGENTS.md table).
  The player, every export route and the saved frame frame and grade alike.
- ffmpeg only as a separate program, through `ffmpeg::tool`; never a bare
  `"ffmpeg"`, never linked.
- No `wgpu` dependency of our own (`iced::wgpu`).
- The project file stays backward compatible: new fields optional, written
  only when not default.
- Portable: no Linux-only path or API without a `cfg` fallback; `.exe` via
  `std::env::consts::EXE_SUFFIX`; folders via `fsutil::app_dir`.
- No help text in panels or bars: explanations are hover hints (`tip`).
  Settings' notes are the user's accepted exception.
- Sound operations start from the original and never stack (pressing again
  replaces).
- A file written whole or not at all (`replace_file`); a file being made
  removed if the work fails (the export's encoder does it on drop).
- Doc comments say why; they sit on the item they describe. No dead code,
  no `allow(dead_code)`, no debug output.
- No footage, personal paths (`<media>`, `~`) or film names in
  code, tests or comments: made-up samples (`testmedia`).

## Before each commit

```sh
export PATH=$HOME/.cargo/bin:$PATH PKG_CONFIG_PATH=../omnix-360-studio/.localdeps/root/usr/lib/x86_64-linux-gnu/pkgconfig
cargo build -q            # no warnings
cargo test -q             # all pass (GPU tests need the GPU)
(cd ../dailix && cargo build -q && cargo test -q)   # it links this library
```

- One local commit per subtask, its message saying why and what was
  measured; never push.
- A change the user will see: the roadmap card (`roadmap` skill) to
  «чекає перевірки». A new idea: a gap in `EDITOR-GAPS.md` and a card.
- A finding not fixed now: a line in `docs/omnix/AUDIT.md` (made again
  if gone), removed when its commit lands; the page goes when empty.
- After a structural change: `AGENTS.md` «Layout» and
  `docs/omnix/ARCHITECTURE.md` say what the code is now (rewrite, don't
  append).
