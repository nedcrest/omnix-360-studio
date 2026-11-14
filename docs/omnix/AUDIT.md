# Audits — the lists to work off

## Small audit of 2026-11-14 (before publishing; the time seam's guards, the sheet's move, the project cards)

Two review agents: Omnix's `src/` since `5e76014` (the moved sheet crate
skipped, its layout read), and Dailix since `b62c67b` with its use of
Omnix's cards and both CIs. No wrong result in the seam's maths and the
export's refactor identical in behaviour; the problems were in the cards.

Fixed (Omnix `300603f`, `8f32afb`, `275aa89`, `ed9177c`; Dailix
`68daa77`, `47da4b9`, `c0b971b`):
- A clicked card stayed hovered (a card gone sends no Exit): lit on
  return and frame-rate ticks through the whole session, in both
  programs. Let go of on leaving; ticks only for a card that moves.
- An edit saved while its strip was being made kept the old frames: the
  strip is dated as the edit it came from.
- A failed strip was final for the session; a spoiled one cached as
  none; two edits outside a trip could share a strip name, a deep path
  pass a file name's length.
- Omnix launched with a project made another edit's strip beside its
  playback.
- 1× in several keys wasn't exact; a doc comment on the wrong test; the
  golden files open to line-end conversion on Windows; `snap` repeated;
  a personal path in a comment.
- Dailix's CI couldn't check a private Omnix out, and named it fixedly;
  both build `--locked`.

Left, and where it belongs:
- **Clippy isn't at zero**: about 31 warnings in Omnix (mostly
  `too_many_lines` in functions not touched since the lint came in), one
  in Dailix (`main.rs` `run`). They go as their functions are next
  touched (`code-rules`); CI can take `-D warnings` once they're gone.
- **`Cut::len` and `Cut::clip_len`** say the same: step 2 of the time
  seam moves `len`'s callers to `edit_len` or `clip_len` and drops it.
- **Dailix's CI follows Omnix's main**, not a pinned commit, and its lock
  resolves Omnix's dependencies at its own patch versions: on purpose
  while the two move together; pin at the first release that needs it.
- **Dailix has no tests** for which edits it follows or the posters'
  order on a trip's card: with its next change there.
- A strip's temporary file can stay behind if the program dies in the
  write's last instant: harmless, never read.

## Small audit of 2026-11-12 (since `40ac31c`: voices 103, Dailix 104, 99b, framing 107)

Two read-only reviews (core; window and Dailix), 23 findings. Fixed the
same night, each with tests:
- groups and sync locks together: a locked title moved twice, a group's
  others stayed when its first cut went, a drag left a locked song behind
  its group — `dcb8e8c`;
- frame keys: moved as one within their lanes, never onto another, held
  past their lanes; `AutoOp::Put` lays a lane down as it is, so «All of
  this clip» / «Whole film» no longer add flat ends — `5e34565`;
- voices: the level kept without a loudness, no offer of the voice a cut
  plays, the marking read once, the last choice wins, a gone cut left out
  of the batch, undo closes the sound menu and the picked key — `fa22908`;
- framing by hand: the drag at any zoom and turn (`Frame::moved`), steps
  closed, ordinary cuts only, keys within the cut, K and Del as meant —
  `b6d05be`;
- Dailix: Esc closes the day's videos, `ALL_DAYS` in one place, times
  read once — Dailix `b62c67b`, Omnix `0de2009`;
- doc comments back on their items, indentation — `4785793`.

Left, with when:
- **With 107b:** a frame off the middle at 100 %, or turned, shows black
  past the picture (the doc says so now); keep the panel's ranges to what
  covers the picture, or zoom to cover, when the box over the picture
  comes.
- **Next time framing by hand is touched:** a wheel notch in the middle
  of a drag closes the drag's step early (two steps).
- «Keep as it is» on the voices offer is an undo step of its own
  (it changes the project); fine unless the user minds.
- Dailix's `Ops::change` (sheet) is 105 lines, old: split when next
  touched.

## Small audit of 2026-11-11 (since `d25b208`: beats, sync lock, groups, the film's end, pieces)

Two read-only reviews, the core and the window, the day after the big
one (the tracker hook said so: 2799 lines, 52 commits). Fixed the same
night, each with tests:
- a selection moves as one op, `ProjectOp::Shift` (a group dragged by
  any but its first clip moved that one twice and left the first; lane
  mates broke apart; a beat pulled moved unrelated clips in several
  steps; a grouped single cut left its song; Alt narrowed a set; a song
  or text couldn't be clicked out of a set) — `d17d7fb`;
- joined pieces keep each one's marks, points, automation, effect keys
  and tracks — `fb4a825`;
- the film's end holds when a lane's placement would push past it;
  trimmed fades stay within; unknown ids said — `822daab`;
- the playing cut found again after deleting a set or joining pieces —
  `1b09915`; Alt+←/→ steps a group's run — `4a511ea`;
- `check()` sees the groups; doc comments back in place — `6608ce2`;
- sync lock rebuilt on the user's choice (cut the song where the
  picture is): inserts, joins apart, songs across the edit, drags back,
  clips under deleted picture — `60e760a`.

Left, each with when it's due:
- **When beats get their next step:** `beats::pull_to` as an op or an
  `omnix beats --pull` option, and `describe` listing a clip's shown
  hits in edit time (an agent can't line a song up now). `onsets` may
  place a hit up to 10 ms early over a loud steady sound (unproven: try
  on the user's songs; then take the onset from the band `rise` chose).
- **Next time `props.rs` is touched:** the property test has none of
  MoveTogether, JoinPieces, Group/Ungroup/GroupLook, TrimToEnd, Shift,
  SyncLock, Beats, and no sync-locked track in its fixture.
- **Next time the timeline is touched:** `OverlayLane::past_end` repeats
  `Project::past_end`; `Timeline::on_frame` uses the playing clip's fps
  for every cut (mixed frame rates snap wrong); the sound lane's cache
  key leaves out the fps (stale beat notches); a second `impl
  OverlayLane` block to merge; the sound menu's hand-counted rows.
- **Next time `project.rs`'s groups are touched:** one helper for every
  clip's (id, group), written out four times now.
- **With 360 footage:** joined pieces keep the first piece's view keys
  only.
- **Ask the user:** Shift+Del deletes the whole selected set, like Del.
- Ops files written with the old `SoundOp::Beats { track }` no longer
  parse (only agents' saved files; none known).

## Audit of 2026-11-10

Five read-only reviews (dead and stale code; bugs in the week's work;
design against `code-rules`; the core API's gaps for an agent; Dailix's
voices and sheet), merged and ranked. Each fix is its own commit, and
its line leaves this file when it's done (the history is in git). Line numbers as of `12ab39c`.

## A. Bugs

All fifteen fixed, each with a test (`git log --grep 'audit A'`, Omnix
and Dailix).

## B. Design (`code-rules`)

All done (B1–B12). The grade's values by name in one `EFFECTS` table;
tracks and items one way each; the timeline's `LaneHead` and one
`active_lane`; pictures copied through `folder_for`, the project's
folder asked once; `grade::luma`, `grade::level`, `MusicClip::local`,
`App::frame_len`; the film's fades in the Transitions panel's own code;
the range, the sound menu and Dailix's Make recipe and speech as tested
functions; no `DerefMut` on `Edit` outside tests (ops through `put`,
what the player learns of a clip through `learn`; `flat` stays a
`serde(skip)` field the core's view keys read); the four long functions
of that week split (older ones are split as they're next touched); the
GPU against the CPU with every grade value, the knee named. Loudness
(the user's decisions, 2026-11-11): `from_sheet` measures the stretch
each cut plays (B11); Dailix compares voices at −20 and Omnix sets the
film at −16, two purposes, two rules, each saying why (B12).

## C. Core API gaps (what an agent can't see or do)

All done (C1–C12). `describe` says the edit's aspect, levels and fades,
a cut's view, settings and sound, a text's look, an item's own time and
keys; the aspect is an op; ops hold values to their ranges and `apply`
warns of what's missing; `looks` lists LUTs and fonts and saves
templates; `text_field` sets one field of a text; `--cuts` takes ids;
the ops schema describes every op and gives each value its range and
default; `omnix frame`, `omnix scopes` and `mixcheck --every` measure
what the player shows; `--ops` prints a measuring command's ops.
Dailix: `about`, `voice use`, and `sheet check` saying a voice that
isn't made.

## D. Stale comments and leftovers

All fixed (Omnix `960deed`, Dailix `998acfa`). No large dead code was
found; every message variant is built and matched; settings are each
read and written.
