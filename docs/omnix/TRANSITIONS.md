# Transitions

The user's idea (2026-10-22): **simple to control and to choose**, and
transitions that look like modern video effects, not "PowerPoint 2004" (the
thing Kdenlive lacked for them). Gap 25 in `EDITOR-GAPS.md`.

## Where it stands (2026-11-09)

Built and checked by the user (2026-11-08/09):
- **Overlap looks** on a join (`Cut::transition`, the incoming cut's; its
  overlap is `Cut::crossfade`): whip pan, zoom punch, spin blur, flash,
  light leak, luma fade, glitch (`transition::Look`). The player draws a
  join's two cuts into pictures of their own and mixes them in a
  transition pass (`player/transition.wgsl`); the export runs the same
  function on the CPU (`transition::colour`/`blend`, in `Dissolve`). A GPU
  test holds them within a level. Sound joins with `acrossfade` as before.
- **A look's own settings** (25g): `Look::knobs` (sliders and choices,
  gl-transitions' values as defaults), stored per join only where moved
  (`Cut::transition_params`), cleared when the look changes; to the shader
  as two vec4s (`transition::Spec`).
- **Edge looks** (25b): an out on the cut before a join, an in on the cut
  after, no overlap (`Cut::edge_out`/`edge_in`: look, seconds, settings):
  fade (black or white), blur, zoom, glitch, flash. Drawn by the same pass
  on one picture (`Cut::edge_at`), never through an overlap. A fade to
  black fades the sound with it (`Cut::sound_fades`); the others leave it.
- **Templates** (25c): a join whole (overlap, out, in) saved by name into
  one library for the program, `<config>/transitions/<name>.json` beside
  the LUTs (the user chose it over per trip or per video), its folder made
  by Omnix at start; put on by `CutOp::Template`, all or nothing.
- **The Transitions panel** (T, or a crossfade's X): the join nearest the
  playhead, following it; tabs Overlap / Edges / Stingers (greyed);
  length in 0.25 s steps; «On every join»; «▶ Play the join» (15 frames
  before to 10 after, then back, paused); the settings in a panel beside.
- **The transitions lane** (25f), under the clip sound, over Effects:
  zones in orange; a rectangle drawn across a join's middle makes its
  overlap, on one side an edge look on the nearer end of that cut
  (`Project::drawn_zone`), with the look chosen last; a click opens the
  zone in the panel, its length's edge drags; it folds.

Postponed: **stingers** (25d, below; the user, 2026-11-09: an extra, both
modes wanted later). Waiting for 360 footage: the **360 looks** (25e).
Not built, still ideas: animated previews on the tiles; a favourite as
the lane's default; a stinger's sound on the SFX lane.

A transition is keyed by the cut after its join (its id is stable); when
a cut can have more than one (stingers), transitions get ids of their own
and a moved cut leaves its transition behind, as other editors do.

## Video transitions (stingers) — postponed

User, 2026-10-22: plain cuts sometimes annoy; a cut can instead be a short
**video transition** — a clip with its own sound placed on the cut: a meme
insert with a sound, white noise + white-noise screen, film burn, etc.

- Its own group, zones drawn **yellow** (not orange); its own tab.
- **Fixed length**, picked per instance: 0.2 / 0.5 / 1 s (the clip trimmed
  or time-fitted to it); per instance its **sound level** and fades.
- From a library: the trip's `06_external` / `07_sfx` (e.g.
  `vfx-white-noise.mp4`, `transition-film-burn-vfx.mp4`); a configured one
  saves as a template like any transition.
- Two ways to place one, **both wanted** (user, 2026-10-22, again
  2026-11-09):
  - **insert**: plays between the two cuts, the edit gets longer by its
    length (a short cut with its own sound);
  - **over the join**: blended over the cut point (screen/add/alpha, e.g.
    film burn and other cinematic overlays), the edit keeps its length.
    Needs video on the overlay lane (milestone «B-roll і групи»).
- Its sound goes onto the SFX lane at once, linked to it (moved and
  deleted with it).

## The library to build from (research 2026-11-08)

The user (2026-11-08): few banal transitions, interesting ones for the
templates folder. A web survey by a background agent; licences checked.

**Source and licences.** gl-transitions
(github.com/gl-transitions/gl-transitions): 206 GLSL transitions, 204 MIT,
two BSD (`InvertedPageCurl`, `StereoViewer`): fine for MIT/Apache; a WGSL
and CPU port keeps the file's `// Author:` and `// License:` lines.
Shadertoy is CC BY-NC-SA by default: ideas only, no code. CapCut,
Premiere, Resolve: names only. Already common on Linux (Kdenlive,
Shotcut), so not ours to repeat: plain dissolve, dip to colour, wipes,
iris, clock, shapes, cube, blinds.

Recommended first (★; all built but the 360 one), the rest for the library later:
- **Camera moves:** whip pan ★ (`WhipPan`, `WhipTilt`), zoom punch ★
  (`CrossZoom`, `ZoomInOut`, `DreamyZoom`, `WarpSpeed`), spin blur ★
  (`WhipSpin`), camera shake (`CameraShake`), tunnel / vortex / swirl,
  curved motion blur (`TangentMotionBlur`). Whip, zoom and spin share
  one blur machine (≈24 samples along the move). A match cut needs the
  frames' motion: far.
- **Light and film:** light leak ★ procedural (`FilmBurn`; no footage,
  no licence), overexposure flash ★ (`Overexposure`, `FlashFade`), lens
  flare sweep (`LensFlare`, `LightStreaks`, `LightRays`), burn-through
  (`BurnThrough`, `UndulatingBurnOut`), photo develop / snapshot, VHS
  tracking / TV off.
- **Flat, per pixel:** luma fade ★ by the picture's own brightness (a
  filmic default instead of a dissolve), glitch / datamosh ★
  (`StripDatamoshGlitch`, `GlitchDisplace`, `GlitchMemories`), morph
  (`Morph`), pixel stretch (`PixelStretch`), ink bleed (`InkBleed`),
  paper tear (`PaperTear`), defocus / bokeh (`BokehDissolve`,
  `DefocusBlur`), shatter / glass (`Shatter`, `FacetedGlass`).
- **360 only** (the sphere's direction in the shader, the CPU twin the
  same sampling): sphere whip pan ★ (spin the view across the sphere,
  the cut in the blur: what no flat editor can do), little-planet zoom,
  sky dive through the nadir or zenith (hides the stick), barrel roll,
  crystal ball (`Sphere`).
- **Sound:** a transition's middle snapped onto a beat (gap 13), a swish
  under each whip or zoom, a glitch or shake driven by loudness.

## 360 looks (25e, waiting for footage)

We hold the whole sphere, so transitions nobody else has: a whip pan
spinning the view across the sphere into the next clip, little-planet
zoom out and in, a sphere roll or tilt through; sky dive through the
nadir or zenith (hides the stick). The tile «Sphere whip» is there,
playing as a dissolve until built.

## Approach

Trial and error (user, 2026-10-22): build the smallest version, try it on
real footage, adjust. Every look exists twice, a Rust function and its
WGSL twin, with a GPU-vs-CPU test; ported looks keep their file's
`// Author:` and `// License:` lines; noise only from the integer `hash`
(a sine hash's last digits differ between GPU and CPU).

## Open questions

- Snap step: always 0.25 s, or 0.25 s by default with Shift for frames?
- An edge look longer than its cut after a trim: it's clamped where it
  plays (`edge_at`), the stored length kept; fine so far.
