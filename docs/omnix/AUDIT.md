# Audits — the lists to work off

## Small audit of 2026-11-12 (since `7a2ebb4`: voices 103, Dailix 104, 99b, framing 107)

Two read-only reviews (core; window and Dailix), 23 findings. Fixed the
same night, each with tests:
- groups and sync locks together: a locked title moved twice, a group's
  others stayed when its first cut went, a drag left a locked song behind
  its group — `cb41c76`;
- frame keys: moved as one within their lanes, never onto another, held
  past their lanes; `AutoOp::Put` lays a lane down as it is, so «All of
  this clip» / «Whole film» no longer add flat ends — `66f4f0a`;
- voices: the level kept without a loudness, no offer of the voice a cut
  plays, the marking read once, the last choice wins, a gone cut left out
  of the batch, undo closes the sound menu and the picked key — `94532e1`;
- framing by hand: the drag at any zoom and turn (`Frame::moved`), steps
  closed, ordinary cuts only, keys within the cut, K and Del as meant —
  `ab2eed5`;
- Dailix: Esc closes the day's videos, `ALL_DAYS` in one place, times
  read once — Dailix `d51f62f`, Omnix `7387d5c`;
- doc comments back on their items, indentation — `dcb19da`.

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

## Small audit of 2026-11-11 (since `ef776a3`: beats, sync lock, groups, the film's end, pieces)

Two read-only reviews, the core and the window, the day after the big
one (the tracker hook said so: 2799 lines, 52 commits). Fixed the same
night, each with tests:
- a selection moves as one op, `ProjectOp::Shift` (a group dragged by
  any but its first clip moved that one twice and left the first; lane
  mates broke apart; a beat pulled moved unrelated clips in several
  steps; a grouped single cut left its song; Alt narrowed a set; a song
  or text couldn't be clicked out of a set) — `6721c8c`;
- joined pieces keep each one's marks, points, automation, effect keys
  and tracks — `0d0ba37`;
- the film's end holds when a lane's placement would push past it;
  trimmed fades stay within; unknown ids said — `b76a354`;
- the playing cut found again after deleting a set or joining pieces —
  `e92d6f6`; Alt+←/→ steps a group's run — `69aa091`;
- `check()` sees the groups; doc comments back in place — `275c56d`;
- sync lock rebuilt on the user's choice (cut the song where the
  picture is): inserts, joins apart, songs across the edit, drags back,
  clips under deleted picture — `6fac766`.

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
its line leaves this file when it's done (the history is in git). Line numbers as of `f13a5e6`.

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

All fixed (Omnix `b17d380`, Dailix `9ad0500`). No large dead code was
found; every message variant is built and matched; settings are each
read and written.
