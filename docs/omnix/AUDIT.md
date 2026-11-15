# Audits — the lists to work off

## Small audit of 2026-11-15 (the time seam, speed, the lanes' scale, paste)

Two review agents: Omnix since `4801410` (Dailix since `4d9f809`: its
README and CI only).

Fixed (`1c8609c`, `931164f`, `6208cc6`, `42cfe7b`, `0cebd43`,
`8917dd5`, `f4c7577`, `12767cd`, `dd5803a`): a point dragged on a
fitted lane ran away; sync lock played a song's stretch twice after a
cut was slowed; sound at speed a few ms long, adding up over cuts; paste
took cuts no op would make; a cut readied ahead kept its old speed; a
fast cut's sound edges snapped off where drawn; a crossfade partner's
drift in clip seconds; the CPU export eased a 360 seam per edit frame,
not per clip frame (as the GPU); doc comments on the wrong items.

Checked, not a bug: `Speed::frame_for` rounds to the nearest clip frame,
and the player shows the frame due by its clock plus half a frame: the
same frame.

Left, and where it belongs:
- `set_speed` settles crossfades but not `Edge.seconds` or the sound
  fades (edit seconds too): a 3 s edge look on a cut set to 4× stays
  stored at 3 s; the maths clamps it. Settle them with the next change
  to `model/ops/cuts.rs`.
- A point's value tag dragged on a fitted lane steps by the value's
  whole range (zoom: ~6 % a pixel): step by the clip's scale in
  `timeline/auto.rs` (`LaneDrag::Value`).
- `Lanes::range` refits a clip on every `y`, every 3 px of a curve:
  work it out once per clip in `draw_clip` and the hit test.
- `OwnPlace` (timeline.rs) and `auto::Owner` have the same edit/own/end
  over (from, start, len, speed): one type when either is next touched.
- `Speed::short` and `describe` repeat «all rates alike»; «at 1× exact,
  else through the seam» is written three times (`Sound::of_cut`,
  `Sound::reads`, `duck::speech`); `Sound::clip_time` works out the
  pieces' edit offsets by hand: a helper in `speed.rs` each, with the
  next change there.
- The source-scan guard (`time_snapshot::around_the_seam`) knows a cut
  by a name ending in `cut`; cuts bound as `c` pass unseen (none today).
- After a paste the playhead stays where it was, before the copies: put
  it on the first copy if he finds it odd.

## Found while building card 31 (2026-11-15)

- An ordinary video's frame comes out of the GPU about 1.9 RGB levels
  brighter than the CPU's (a flat grey clip, 64×36, every grey from 19
  to 152; mean, not a pixel or two). Not speed's: the same at 1×. Where
  it belongs: the flat clip's YUV→RGB on the two paths (range or
  matrix), with a test over a grey ramp at 0 and at 255 — the next time
  `render.rs` or the flat decode is touched.

## Small audit of 2026-11-14, third (the UI review's steps 3–9, the guide's generators)

One review agent: Omnix since `b1c87a6`, Dailix since `c302feb`. The
story autosave sound (switching trips saves first); no data lost.

Fixed (Omnix `324a7fe`, `2428fe3`; Dailix `95a95ef`, `434c474`,
`99e0238`, `55cce8e`, `3654473`, `7afe3d8`, `458ad8e`, `4d9f809`):
- A second «→ Omnix» while the first edit was made off the window's
  thread made a copy; two edits at once could take one folder.
- A trip trashed while videos were added to it, or an edit made of it,
  came back half-made.
- A day opened under a filter didn't scroll to it.
- The commands page showed `&lt;` inside code spans.
- A hovered tile hidden by a filter or a rating, or from another trip,
  kept its buttons; a filter click opened a clip, or kept a hidden one;
  a story saved by itself wiped other errors; the toggle on a click
  untested and its tooltip wrong about the key; videos added to a
  loaded sheet's trip didn't show till another trip was opened.
- Dailix's guide tests tie its CI to Omnix's main: the push order is in
  `docs/PUBLISH.md`.

Left, and where it belongs:
- The ratings' facts (2/1/0, symbols, words, colours, keys) written in
  several places: one table per `Rating` with the next change to them.
- `step_from` with a step past ±1 can say none while clips show (only 0
  and ±1 are asked today).
- `menu::on_screen` adds every scrollable met before the button that
  holds its point, not only those it's inside: right for today's
  layout; push and pop scrollables if the layout grows one above another.
- An open menu stays where it was placed if the page reflows under it
  (a resize, cards read again): close it on those.
- `make_poster` checks the clip twice.

## Small audit of 2026-11-14, second (adding videos, delete, the welcome, the UI review's start)

One review agent: Omnix since `b84a3e3`, Dailix since `e0ccb40`. Omnix's
UI changes clean; the problems were in `dailix add` and the sidecars.

Fixed (Omnix `d9da2fb`; Dailix `c302feb`, `61664b8`):
- A.MOV added beside A.MP4 overwrote its sidecar: A.MP4 vanished from
  the sheet. Refused now (`sidecar_source`).
- Span names rounded per part (59.6 s «0m60»), two spans could share a
  name and the second be skipped: rounded once, tenths kept.
- `day_of` panicked on a made date that wasn't ASCII and took any ten
  characters as a day; `--day ../x` wrote outside the trip: `is_day`.
- A story that couldn't be saved was still dropped by opening another
  trip or closing the window.
- One bad file lost the lines of those added before it; `summary` read
  its own strings back; `@` in a file's name, `nan` as a time; ffmpeg
  without `-nostdin`; two adds of one file shared a part file; sidecars
  written in place.
- A trip trashed while its posters were being made got its folder made
  again; an open sheet of it stayed.

Left, and where it belongs:
- «Close it in Omnix first» before trashing an edit is only words: an
  open Omnix saving it brings its folder back. When Omnix can say what
  it has open.
- The file system asked in `view` (the welcome's `is_dir`, Omnix's
  start cards per card per frame); `first_run` worked out twice in
  `home()`: when those functions are next touched.
- A video without a size in ffprobe gets «nullxnull» as its resolution.

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
