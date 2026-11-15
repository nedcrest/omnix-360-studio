# Omnix 360 Studio

A home-video editor for Linux, for ordinary and 360 footage (at first
"LightCut, a bit more"; the vision as written on 2026-10-22). A fork of
[rOsmo360](https://github.com/ravelund/rOsmo360) by Vibing with Ravelund,
whose 360 playback it keeps. 360 is one kind of clip
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
- The user, 2026-10-23: "we almost repeat any editor, but Kdenlive I don't
  want to start and do things in, and Omnix I do. And I don't want to
  dual-boot into Windows every day for DaVinci. It really makes me want to
  do things with video and sound." That is the measure for every feature:
  it should be pleasant to use, not merely present.
- **Lanes are a fixed, named set that is enough** (user, 2026-10-23):
  - Text, Overlay, the cuts, Clip sound, Music, SFX, later Voice;
  - not tracks added and grouped at will;
  - easy to work in and easy to automate.

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

Added in the fork (to 2026-11-09):
- an operations core (`OPS-API.md`): every change an op, so a script or
  an AI edits as the player does;
- sound lanes, per-clip loudness, auto-duck, J/L cuts, the mix check;
- the bin, insert at the playhead or the nearest join, markers;
- transitions: a library of non-banal looks across a join, edge looks,
  their settings, templates, a transitions lane;
- Dailix, the contact sheet and trips (`../dailix`), sharing Omnix's
  library; the start window, «New…» and the projects place in both;
- the live roadmap page.

Detail: `CONTEXT.md`.

## Where it goes

The order of work is the milestones in `ROADMAP.md` (cards, in order) and
`EDITOR-GAPS.md` «Milestones» (detail). Done or mostly: sound for Kos,
the timeline's ease, material and the bin, transitions. Next (user,
2026-11-09): **Ефекти, колір і розумний звук**: effects and colour,
style looks and animation for text and pictures, object tracking,
speech found and transcribed in Dailix, a cleaner voice, auto-duck in
the original videos too. Then who is in the frame, B-roll and groups,
framing, multicam, speed and automation, 360 quality, export, the AI
edit (`AUTO-EDIT.md`; the user's main wish, last on purpose: first
Omnix has to do everything an edit needs), other platforms.

Further out, no milestone yet:
- speaker-follow (direction of arrival plus active speaker);
- visual search over footage;
- optional cloud models through a user-supplied key;
- bridges: flat clips (exists), OpenTimelineIO, Kdenlive.

## Stack (as built)

| Part | Choice | Why |
|---|---|---|
| Core | Rust | upstream's; speed for 8K frames; telemetry-parser for DJI data |
| UI | iced 0.14 | upstream's; one window, custom canvas timeline |
| GPU | wgpu through iced's re-export | stitch, grade and overlays in shaders, mirrored on the CPU |
| Decode / encode | ffmpeg CLI (CUDA, Vulkan, VA-API, software) | a separate process keeps licences simple |
| Material | Dailix (Rust, `../dailix`) | the contact sheet, trips and their videos, ingest and analysis to come (speech, voice); footage-agent (Python) the older pipeline, its models (voice cleaning, Whisper) to move over |

The machine is an RTX 3060 12 GB on X11 and KDE. CUDA decodes both 3840²
lenses at about 4× real time. Osmo 360 II's 8K50 is unverified.

## Open questions

- Does telemetry-parser read the Osmo 360 II calibration?
- Is 8K50 HEVC 10-bit real time on the 3060, or are proxies required?
- How does a project hold links between items (groups, connected clips)
  while staying backward compatible? This is the B-roll milestone's design question.
