# Project cards that show what's inside (gap 110, 2026-11-14)

The user: the start pages' recent projects should remind him what each
holds. Today Omnix's start lists recent edits as text, Dailix's Home
shows trips as 300 px cards with one still, and Dailix's «→ Omnix»
question lists a day's edits as names and dates. One answer for all
three. His ideas: (a) a grid, each card a ~10-frame animation on hover;
(b) bigger cards, a 2×2 mosaic of frames changing to the next ones
(~16 per project) with a crossfade or a wave, not blinking; and better
framing (depth, pseudo-3D panels).

## What others do (web research, sources in the session's report)

- **Pro editors show one still per project** (DaVinci Resolve's Project
  Manager, iMovie's midpoint frame, Premiere's Home list, Kdenlive 25.12's
  welcome list). Motion lives in bins only, as **hover-scrub**: the frame
  follows the pointer across the thumbnail (Final Cut's skimming,
  Premiere's Hover Scrub, Resolve's media pool, Avid's 170-frame
  hoverscrub); leaving the card returns to the poster.
- **Consumer apps animate** (Google Photos' cinematic memories, Apple TV's
  focus lift and tilt, YouTube's inline playback after a hover delay —
  annoying enough that people install blockers).
- **No editor has a self-changing mosaic on its start page**: (b) would
  be Omnix's own, with no pattern to copy.
- **Changing tiles calmly**: Windows Live Tiles changed at random
  intervals so a screen never flipped at once. Calm numbers: one tile at
  a time, a 600–900 ms ease-in-out crossfade, 4–7 s between changes with
  ±1–2 s jitter, each card starting offset. A wave (120–200 ms stagger)
  suits a one-off reveal; repeated, it looks mechanical.
- **Frames, not GIFs**: ~16 frames pulled once (at saving or import)
  into a small strip (JPEG/WebP, ~320 px a frame) in the project's cache;
  the start page never decodes video. A GIF has 256 colours, is bigger and
  can't be driven frame by frame.
- **Motion that starts by itself** needs a way to stop it (WCAG 2.2.2):
  a setting (animated / on hover / off), paused when the window isn't
  focused and for cards out of view.
- **Depth on a dark theme**: shadows vanish on dark grounds; depth comes
  from a slightly lighter card surface, a 1 px inner highlight (white at
  6–10 %) and a large soft shadow (black 40–60 %). A tilt is gimmicky with
  a mouse unless barely there (2–3°, the hovered card only). A **stack of
  sheets** behind a card (1–2 offset, ±2°) says «a collection» without
  motion: fits a trip. Avoid neumorphism (fails contrast), glows, bounce,
  scaling past 1.04.

## Options

- **A. Still + hover-scrub** (the pro editors' way). Cards 280–320 px,
  16:9, a poster at rest; hover scrubs 16 frames by pointer position, a
  thin progress line under it. Calm, dense (4–5 columns), nothing moves
  unasked; shows nothing until the pointer gets there.
- **B. Living mosaic** (the user's idea b). Cards 380–440 px, a 2×2
  mosaic of 16 frames, one tile crossfading every ~5 s. Shows what's
  inside at a glance and is distinctive; moves at rest, frames small,
  3 columns.
- **C. Hybrid** (the research's pick). A still 2×2 mosaic at rest (four
  telling frames: for a trip one per day); the hovered card's tiles
  crossfade one by one through its 16 frames, and the pointer can scrub
  them. Stack-of-sheets edge for Dailix trips, flat for Omnix edits.

The user chooses; the frames' source and cache are the same for all
three (a strip per edit and per trip, made when it's saved).
