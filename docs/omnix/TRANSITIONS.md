# Transitions — feature idea

User's idea, 2026-10-22. Not started. The thing Kdenlive lacked for them:
**simple to control and to choose**, and transitions that look like modern
video effects, not "PowerPoint 2004".

## What exists upstream

rOsmo360 has one transition: the crossfade. Shift-drag a cut's start edge back
over the cut before (or the crossfade button), the overlap is drawn as an X,
`Cut::crossfade` holds its length in seconds. Player: both cuts decode at
once and blend in the shader (`App::incoming`, `gpu::Pipeline` second layer).
Export: `dissolve::Dissolve` (a `FrameSink`) blends the tail into the head
linearly; sound joins with `acrossfade` (qsin). So two layers playing at once,
mirrored GPU/CPU, is already solved. Transitions generalise the blend.

## Interaction

- **A Transitions lane** (user, 2026-11-01; gap 25f): a thin lane above the
  Effects row, folding open and closed like the automation groups (gap 39).
  Drag a rectangle across a join on it, in **0.25 s steps**: it's filled at
  once with the default template (the last used, or a favourite), drawn
  **semi-transparent orange** (a stinger **yellow**). No separate transition
  mode: the cut row stays as it is.
- A stinger's sound goes onto the SFX lane at the same time, linked to it
  (moved and deleted with it, levelled and faded there like any clip).
- Dragging one video over another (overlap, as the crossfade does today)
  marks the overlap orange automatically.
- **Click an orange zone** → a side panel (right, like Effects) with transition
  presets, each shown as an animated preview. Click a preset → its parameters
  appear under it.
- **Project templates:** a configured transition can be saved as a named
  template into the project folder (e.g. `<project>/transitions/<name>.json`)
  and picked for any other zone, ready as is.

## Quick ways to add one (user, 2026-10-23)

Dragging zones in a mode is the precise way. Most of the time it should be
one action, with the default transition (or the last template used):
- **At the playhead:** with the playhead on or near a join, the T key or a
  button puts a transition on the nearest join (`Edit::nearest_join`, which
  the bin's «на склейку» already uses).
- **Between two selected clips:** select two neighbouring cuts (needs
  multi-select, gap 8; until then: the selected cut and the one after it),
  then «Додати перехід».
- **On every join** of the selection or of the whole edit, with a chosen
  template («автоматичні склейки»). One undo step. Joins too short for it
  are skipped and named.
The same three work for stingers («перебивки»), placed in their insert or
cover mode. All of them fill the Transitions lane; the rectangle's edges
are there for fine-tuning.

## Two kinds, detected automatically

1. **Overlap transition** — the two cuts overlap in time; both pictures are
   visible during it (dissolve, wipe, zoom-through, light leak, whip pan…).
2. **Edge transition** — no overlap: an **out** on the end of clip 1 and/or an
   **in** on the start of clip 2 (fade to/from black or white, blur out/in,
   zoom out, glitch out…). Each half belongs to its own cut.

The zone's shape tells which: an overlap is one zone across both cuts; edge
zones sit on each side of the cut point.

## Video transitions (stingers) — a separate group

User, 2026-10-22: plain cuts sometimes annoy; a cut can instead be a short
**video transition** — a clip with its own sound placed on the cut: a meme
insert with a sound, white noise + white-noise screen, film burn, etc.

- Its own group in the preset panel; zones drawn **yellow** (not orange).
- **Fixed length**, picked from options per instance: 0.2 / 0.5 / 1 s (the
  stinger clip is trimmed or time-fitted to it).
- Per instance: **sound level** (dB, like volume points), maybe its own
  fade.
- Stingers come from a library — the project's (`06_external/`, `07_sfx/` in
  the user's footage-agent layout, e.g. `vfx-white-noise.mp4`,
  `transition-film-burn-vfx.mp4`) — and a configured one saves as a project
  template like any transition.
- Two ways to place one, **both offered as an option per instance** (user, 2026-10-22):
  - **insert**: plays between the two cuts, the edit gets longer by its length;
  - **cover**: plays over the cut point (blended: screen/add/alpha, e.g. film
    burn), the edit keeps its length.
- Builds on the overlay track (pictures/video over the main picture) and the
  music/sound mix that already exist upstream.

## Looks

Our own, built from good examples. 360 footage is a unique source of
transitions nobody else has, because we hold the whole sphere:

- whip pan (fast yaw spin with motion blur) from one clip into the next,
- little-planet zoom-out → zoom-in into the next clip,
- sphere roll / tilt-through.

Flat ones: dissolve with a curve, dip to colour, directional wipe with soft
edge, zoom blur, light leak, film burn (the user already uses
`transition-film-burn-vfx.mp4` overlays), glitch, luma fade.

## Approach

Trial and error (user, 2026-10-22): build the smallest version, try it on real
footage (the Kos project), adjust. Don't design every detail up front.

## Implementation notes

- Per upstream `AGENTS.md`, every transition exists twice: a WGSL function in
  the shader and a Rust one on the CPU path (`Dissolve` today), plus a
  GPU-vs-CPU test like `gpu_lut_matches_the_cpu`.
- Data model: `Cut::crossfade: f64` becomes the length of an overlap
  transition; add a transition spec (`kind`, params, optional `template`) for
  the overlap, and `out`/`in` specs for edge transitions. New fields default
  so old projects load unchanged (public format).
- Previews: render each preset with our own renderer on two built-in sample
  frames into a small animated image at build time or first run — always
  matches what the export will do, no hand-made GIFs.
- Sound: overlap keeps `acrossfade`; edge transitions fade sound out/in with
  the picture (optional parameter).
- Templates live in the project folder so they travel with the project; maybe
  later a user-wide library like the LUT library.

## Open questions

- Snap step: always 0.25 s, or 0.25 s by default with Shift for frames?
- Where do edge zones go when a cut is trimmed shorter than the zone?
