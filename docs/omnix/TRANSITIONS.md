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

- **T** (free key; check again before binding) toggles transition mode, plus a
  button in the transport row.
- In transition mode, the end of one cut and the start of the next get
  handles; drag them to mark a transition zone, in **0.25 s steps**. The zone
  is drawn **semi-transparent orange**.
- Dragging one video over another (overlap, as the crossfade does today)
  marks the overlap orange automatically.
- **Click an orange zone** → a side panel (right, like Effects) with transition
  presets, each shown as an animated preview. Click a preset → its parameters
  appear under it.
- **Project templates:** a configured transition can be saved as a named
  template into the project folder (e.g. `<project>/transitions/<name>.json`)
  and picked for any other zone, ready as is.

## Two kinds, detected automatically

1. **Overlap transition** — the two cuts overlap in time; both pictures are
   visible during it (dissolve, wipe, zoom-through, light leak, whip pan…).
2. **Edge transition** — no overlap: an **out** on the end of clip 1 and/or an
   **in** on the start of clip 2 (fade to/from black or white, blur out/in,
   zoom out, glitch out…). Each half belongs to its own cut.

The zone's shape tells which: an overlap is one zone across both cuts; edge
zones sit on each side of the cut point.

## Looks

Our own, built from good examples. 360 footage is a unique source of
transitions nobody else has, because we hold the whole sphere:

- whip pan (fast yaw spin with motion blur) from one clip into the next,
- little-planet zoom-out → zoom-in into the next clip,
- sphere roll / tilt-through.

Flat ones: dissolve with a curve, dip to colour, directional wipe with soft
edge, zoom blur, light leak, film burn (the user already uses
`transition-film-burn-vfx.mp4` overlays), glitch, luma fade.

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
- Does T toggle a mode, or just add a default transition at the cut nearest
  the playhead (and the handles are always there)?
- Where do edge zones go when a cut is trimmed shorter than the zone?
