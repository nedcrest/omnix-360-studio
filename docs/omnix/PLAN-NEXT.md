# The «Далі» cards planned together (2026-11-11)

The user asked to plan the four cards in «Далі» as one piece of work, so
that doing them one after another doesn't mean redoing code: gap 100
(outside material), 25d (stingers), 31 (speed) and 31c (the speed curve
window). On the way he split 25d in two himself and added copy and paste
(gap 102). This is the analysis; the order is his to approve.

## What each thing really is

| Thing | What it is | Where it lives | Reused how |
|---|---|---|---|
| Outside material (100) | files from a link or dragged in, with their licence | the trip's folders (`02_audio/music`, `07_sfx`, `06_external`, pictures) | the bin shows them already |
| Stinger (25d, transition) | a short video with its own sound on a **join**, blended over it (light leaks, film burn) | the program-wide template library, `<config>/transitions/` (gap 25c), not a trip | a template, one click on any join of any film |
| White noise (25d) | a transition made by the program: noise on the picture, hiss in the sound | a `Look`, no file | like any look |
| Insert / cutaway | a **piece of footage** (a funny phrase) put in many places | the trip's footage; Dailix already has the role «перебивка» (`cut`, cutaway) for such marks | copy and paste (102), or the mark dropped from the bin |
| Speed (31, 31c) | how fast a cut reads its clip, constant or as a curve | `Cut` | — |

Two things share the word «перебивка»: Dailix's role (a cutaway clip)
and card 25d (a transition clip). They are different and should be named
apart: 25d → «стінгери» (stingers), «перебивка» stays Dailix's cutaway.

## Dailix or Omnix

- **Dailix brings material in and marks it**: camera import today; links
  (yt-dlp as a CLI program, never linked) and dragged-in pictures for gap
  100, the conversion (JPEG/WebP → PNG), the `LICENSES.md` line. A
  cutaway is marked there with its role, as now.
- **Omnix edits**: copy and paste, cutting a stinger out of a file, speed.
  The bin's full-screen player already has I/O points (`bin_player.rs`),
  and the marks it writes are the same sheet Dailix reads, so a cutaway
  marked in either shows in both. Cutting stingers in Dailix would mean a
  second player with I/O points there.

## What the cards share (build once)

1. **The time seam.** Edit time ↔ clip time is worked out by hand today:
   `offsets(` in 23 places over 14 files, a cut's `len()` in about 50.
   Speed changes every one of them. Put them behind `Cut` methods (its
   length on the edit, clip time from edit time and back) first, with
   speed 1 so nothing visible changes and the tests prove it. Whatever is
   built after (paste, stingers) uses the seam from day one.
2. **A fragment.** Selected cuts, sound clips, text and pictures as
   project JSON, times relative to the first. Copy writes one (also to the
   system clipboard, `iced::clipboard`), paste is one op
   (`ProjectOp::Paste { fragment, at }`: one undo step, agents can use it,
   groups kept with fresh ids). A stinger template carrying a clip is the
   same shape later; so is a saved «insert» library, if ever wanted.
3. **A join's own sound.** Joins only crossfade the cuts' sound today.
   White noise needs a hiss, a stinger its file's sound. One place in the
   mix (player and export), built for the first and reused by the second.
4. **Video on the overlay lane (card 4, «Відео на доріжці накладок»).**
   A stinger over a join is a video blended on top (screen/add for light
   leaks): an extra decoder in the player and the export. B-roll needs the
   same. Building stingers before it means a join-only decoder, redone
   for B-roll later.
5. **Speed stored as a curve from the start.** 31 is a constant, 31c a
   curve; store one key for a constant so 31c changes no format. Sound:
   `atempo` (pitch kept, chained under 0.5×) or `asetrate` (tape) for a
   constant; a curve in pieces of constant tempo.

## Where it would be redone if done in the wrong order

- Stingers over a join before card 4: the decoder twice.
- Paste or stingers before the time seam: their time maths revisited for
  speed.
- Speed as a plain number, then a curve: a format change and a migration.
- A stinger made from a downloaded file without its licence: templates
  are reused in every film, so the licence line must travel with the
  template (`source`, `licence`), not stay in one trip's `LICENSES.md`.

## A proposed order

1. The time seam — small, nothing visible.
2. Copy and paste (102) with the fragment — small, useful at once; covers
   cutaways.
3. Outside material in Dailix (100) — independent of Omnix's code; can
   go any time.
4. A join's own sound and the white noise transition (25d, first half) —
   small.
5. Video on the overlay lane (4), then stingers over a join (25d, second
   half): «Save as transition…» from the bin player's I/O, the template
   carrying the file, its sound and licence; light leaks first.
6. Speed (31), stored as a curve; then the curve window (31c).

Speed has no dependency but step 1: if it hurts more than stingers, 6 can
follow 2.

## Constraints that apply

No vendor SDKs (yt-dlp is run as a program). Downloaded footage is never
committed. Personal footage and paths stay out of code and tests.
