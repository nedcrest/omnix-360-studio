# Omnix 360 Studio

A Linux-native studio for DJI 360 video and a **simple home-video editor**
("LightCut, a bit more"). Built as a local fork of
[rOsmo360](https://github.com/ravelund/rOsmo360). 360 is one kind of clip
beside ordinary phone, Action 2 and Neo 2 footage. The default mode is plain
editing, and 360 framing turns on for a selected 360 clip (`EDITOR-GAPS.md`
gap 10).

**DJI only** (decided 2026-10-22). The user's gear is DJI: Action 2, Neo 2
and Osmo Mobile 7P. The 360 camera will be an Osmo 360 or Osmo 360 II.
Insta360 is out of scope.

## Why

- DJI Studio and Insta360 Studio exist only for Windows and macOS.
- Free DaVinci Resolve on Linux cannot decode H.264/H.265 (patent licensing),
  so its 360 plugins are out of reach. The user chose to build their own;
  don't re-pitch Resolve or Kdenlive.
- Kdenlive was too complex for fast family editing. The real test is the Kos
  2026-08 film, whose open problems (`den1-TODO.md` there) drive the order of
  work.
- A personal project first; a polished version on GitHub maybe later.

## Core ideas

- **Edit fast, from marked material.** footage-agent (Python, `footage-agent's folder`)
  ingests, analyses and marks footage on a contact sheet. «→ Omnix» turns the
  marks, notes and chosen cleaned voices into an Omnix project. Omnix is where
  the cut is finished.
- **Virtual cameras in a 360 clip.** A 360 clip is a set, not a clip to
  rotate. Inside it we place views with their own yaw/pitch/roll/FOV paths
  (keyframes, follow the stick, follow a colour) and cut between them. AI
  places and drives them later.
- **Easy and accessible.** Big targets, every drag also has a button or a
  key, and nothing relies on colour alone (`TIMELINE-UX.md`).

## Principles

- No vendor SDKs, no registrations. All parsing, calibration and stitching is
  open code anyone can build.
- Upstream's rules hold (`AGENTS.md`): CPU and GPU paths match, with tests;
  ffmpeg only as a separate program; no direct `wgpu` dependency; the
  project JSON stays backward compatible; no help text in the UI.
- Footage never goes into git.

## Where it stands

From rOsmo360 (upstream) we already have:
- stitching with DJI's own calibration;
- gyro stabilisation and rolling-shutter correction;
- reframing keyframes;
- ordinary videos and pictures as cuts;
- a music lane with volume curves, crossfades, keyframed effects, LUTs and
  D-Log M;
- text and picture overlays;
- GPU export, plus DNxHR for Resolve.

Added in the fork:
- the bin;
- insert at the playhead or the nearest join;
- cleaned voice in place of a clip's sound;
- the footage-agent bridge;
- the live roadmap page.

Detail: `CONTEXT.md`.

## Where it goes

The order of work is nine milestones, kept current in `ROADMAP.md` (cards)
and `EDITOR-GAPS.md` (detail):

1. **Звук для Косу.** Sound lanes, per-clip loudness, auto-duck under speech,
   and a true-peak limit.
2. **Зручний таймлайн.** Two modes, big targets, click routes, snapping,
   skimmer, markers.
3. **Матеріал і бін.** Preview in Omnix's viewer, grid, ratings from the
   contact sheet, new project with folders.
4. **Розуміння матеріалу.** Speech transcription and who is in the frame,
   with local models in footage-agent.
5. **B-roll і групи.** Video on the overlay lane, group/ungroup, connected
   clips, multi-select.
6. **Ритм і переходи.** Cuts on the beat, transitions and stingers.
7. **Колір і текст.** Levels, cinematic bars, glow, vignette shapes,
   subtitles, text-based editing.
8. **Якість 360.** Ideas from OpenOSV: easing presets, a bundled D-Log M
   LUT, lens protectors, a better seam beside the stick, Osmo 360 II.
9. **ШІ-монтаж.** An AI edits from the contact sheet's brief by written
   rules (`AUTO-EDIT.md`). It is the user's main wish, but comes last on
   purpose: first Omnix has to do everything an edit needs.

Further out, no milestone yet:
- speaker-follow (direction of arrival plus active speaker);
- click/text tracking ("follow the dog");
- visual search over footage;
- an AI rough cut that writes the project JSON;
- optional cloud models through a user-supplied key;
- bridges: flat clips (exists), OpenTimelineIO, Kdenlive.

## Stack (as built)

| Part | Choice | Why |
|---|---|---|
| Core | Rust | upstream's; speed for 8K frames; telemetry-parser for DJI data |
| UI | iced 0.14 | upstream's; one window, custom canvas timeline |
| GPU | wgpu through iced's re-export | stitch, grade and overlays in shaders, mirrored on the CPU |
| Decode / encode | ffmpeg CLI (CUDA, Vulkan, VA-API, software) | a separate process keeps licences simple |
| Material and AI | footage-agent, Python | ingest, analysis, voice cleaning, Whisper; the model ecosystem lives there |

The machine is an RTX 3060 12 GB on X11 and KDE. CUDA decodes both 3840²
lenses at about 4× real time. Osmo 360 II's 8K50 is unverified.

## Open questions

- Does telemetry-parser read the Osmo 360 II calibration?
- Is 8K50 HEVC 10-bit real time on the 3060, or are proxies required?
- How does a project hold links between items (groups, connected clips)
  while staying backward compatible? This is milestone 4's design question.
