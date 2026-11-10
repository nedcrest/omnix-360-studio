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
  And an agent must see and find it: `omnix describe` says the new value
  where it's set, and names, keys and ranges an op takes are listed by a
  command (`ops-schema` gives the shape only; e.g. `omnix looks`
  lists the looks' settings). Transitions shipped without both at first
  (2026-11-09).
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

## Habits that grew into a refactor

The timeline package (2026-11-06/07, four steps, two days with the
user's checks) and the bugs found in live checks came from five habits,
each easy to miss while building fast. Hold to their opposite as you
write; it costs minutes now instead of days later.

- **One question, one answer.** What's under the pointer is one function
  returning an enum (`Timeline::hit` → `Hit`); a press, the hover's
  pointer shape (`Hit::pointer`) and a right-click all read it. The same
  for any question asked in two places: which panel owns a place, whether
  a drag may start, what a key does.
- **The second copy is the moment to share.** Before drawing a block,
  handle, strip, tag or mark, look for the helper the other lanes use
  (`coloured_block`, `selected_block`, `trim_handles`, `crossfade`,
  `name_tag`, `folded_strips`). Writing a second copy of anything over
  three lines: make the helper now, in the same commit.
- **A view shows the model; it doesn't restate it.** A view struct holds
  the model's item (`Block::cut`, `MusicBlock::clip`, `OverlayBlock::item`)
  and only what the player loads or works out (positions, waveforms,
  thumbnails, labels). Never a model field copied under a new name. One
  selection, found once per drawing (`Selection::picked`).
- **A derived value is a method.** An expression like `start -
  sound_before` written twice is a method (`Block::sound_span`, `len`):
  the waveform, fades, edges, line and hit test then can't disagree (the
  volume line once ran to the picture's edge, not the sound's).
- **Don't store what follows from something else.** A point stored at a
  clip's edge stayed behind when the edge moved («Show volume line» put
  two). Store what the user chose; derive what follows (a line runs flat
  past its last point to wherever the edge is).

And two of process:

- **Functions stay short.** A handler does one kind of event; a canvas's
  `update` dispatches to a press, a drag, a release. Past ~80 lines, or a
  match arm past ~15, split it before adding to it.
- **Clean as you go, not in a package later.** Touching a function that
  breaks a rule above: fix that first, in its own commit, then build the
  feature on it. A finding too big for now goes in `AUDIT.md` at once.

## The window

- **Places shared on screen say who wins and what closes them**, in one
  place (the left side: Info and Settings over the Text panel; a new
  selection closes Info).
- **Sizes relative to the box, tried at both ends.** A thumbnail, a tag
  or a label fits the block it's in (lane height and block width as
  limits, its own aspect); never "only when wider than three heights".
  Try a new piece at the smallest and the largest lane and window.
- **Checks on the test project**: `<drive>/DAILIX/2026-08_test` (CONTEXT),
  every lane filled; add to its edit what a new feature needs to be seen.

## Done means done

The audit of 2026-11-10 took about twelve hours to work off, and half of
it was steps skipped while building fast: a setting with no op or no
`describe` line, a rule in a handler with no test, a function grown past
80 lines. So a card goes to «чекає перевірки» only when, for what it
added (user, 2026-11-11):

- [ ] every change to the project is an op, range-checked, with a test;
- [ ] `omnix describe` says the new value; its names, keys and ranges
      are listed by a command (`looks`, `ops-schema`);
- [ ] every rule it added (what a key, a menu, a drop does) is a pure
      function with a test, the window only calling it;
- [ ] no function it touched is over 80 lines (clippy's `too_many_lines`
      says so: on in `Cargo.toml`, threshold in `clippy.toml`);
- [ ] CPU and GPU agree where it draws, with a test;
- [ ] no warning in a file it touched, old ones cleared in their own
      commit — except `too_many_lines`, which holds for the functions
      it touched (the old long ones wait till they're touched).

The 33 long functions there were when the lint came in are split as
they're next touched, not in a package.

## Audits: small and often

- Every two or three days of features, a small audit: one or two review
  agents over what changed since the last (`git log`), not five over
  everything. An hour, not half a day.
- Its findings sorted, not all worked off at once: bugs now; a gap in
  the core API when a feature needs it; a refactor when its file is
  next touched. Two or three hours at most; the rest stays in
  `AUDIT.md`, each line with where it belongs.

## Before each commit

```sh
export PATH=$HOME/.cargo/bin:$PATH PKG_CONFIG_PATH=../omnix-360-studio/.localdeps/root/usr/lib/x86_64-linux-gnu/pkgconfig
cargo build -q            # no warnings; never commit what doesn't build
cargo test -q             # all pass (GPU tests need the GPU)
(cd ../dailix && cargo build -q && cargo test -q)   # it links this library
```

- One local commit per subtask, its message saying why and what was
  measured; never push.
- A mass edit (a script, sed): assert how many places it changed, build,
  read the diff before the commit; the compiler finds what it missed,
  not what it changed wrongly.
- A change the user will see: the roadmap card (`roadmap` skill) to
  «чекає перевірки». A new idea: a gap in `EDITOR-GAPS.md` and a card.
- A finding not fixed now: a line in `docs/omnix/AUDIT.md` (made again
  if gone), removed when its commit lands; the page goes when empty.
- After a structural change: `AGENTS.md` «Layout» and
  `docs/omnix/ARCHITECTURE.md` say what the code is now (rewrite, don't
  append).
