# Changelog

## 0.2.0 — with Dailix's first release (2026-11-17)

Dailix, the contact sheet before the edit, is now released beside Omnix
([its releases](https://github.com/nedcrest/dailix/releases)); each
finds the other installed beside it, or as two AppImages in one folder.

- **Dailix: TMP sorted into trips** (gap 117): TMP's videos by the day
  they were filmed, picked by click, Ctrl, Shift or arrows, filed into a
  trip (or a new one) and its **parts**, a trip's own groups of clips
  across days. A part's edit in Omnix (`omnix from-sheet --part`), a
  part alone in Omnix's bin and in Dailix's sheet, a group of clips put
  into a part from the sheet ([the guide](docs/guide/tutorial-0-bring-in.md)).
- **Dailix: the originals into the archive, TMP freed** (gap 119): each
  copy checked by its BLAKE3 hash, the original sent to the trash only
  after that; «Archive all and free TMP», «Remove picked from TMP»,
  «How Dailix works».
- **The bin**: a clip's rating, segments and note from Dailix on its
  tile; **E** and **W** add the bin player's clip at the end or the
  playhead; a trip's `09_luts` in a cut's LUT list.
- `omnix describe` says a speed curve's ramps.

## 0.1.0 — Omnix's first release (2026-11-16)

Omnix numbers its own versions from 0.1.0: it is its own program, not a
release of rOsmo360 (whose own 0.8.0 came out on 2026-11-15).

Omnix grew from [rOsmo360](https://github.com/ravelund/rOsmo360) 0.7.1
(a DJI Osmo 360 player and timeline editor, by Vibing with Ravelund) into
a home-video editor, its 360 playback kept as it was, in
three weeks, built with Claude ([`docs/AI-DRIVEN.md`](docs/AI-DRIVEN.md)).
Every item below has its section in
[`docs/omnix/EDITOR-GAPS-DONE.md`](docs/omnix/EDITOR-GAPS-DONE.md).

### Speed, the speed curve, copy and paste

- **Speed**: a cut slowed down or sped up from ¼× to 4× (its right-click
  menu), its sound with it, keeping its pitch or like tape; in the
  player and both exports (`CutOp::Speed`, `SpeedCurve`, `TapePitch`).
  Under it the time seam: every clip ↔ edit time through one place.
- **The speed curve**: a window over the timeline where a cut's speed
  (or several side by side) is drawn as a curve with ramps between its
  points, played at that speed before it's applied, with a playhead of
  its own; one undo step ([the guide](docs/guide/speed.md)).
- **Copy and paste** cuts: Ctrl+C, Ctrl+V at the playhead (the cut there
  split), Ctrl+Shift+V at the nearest join (`CutOp::Paste`).
- **Automation lanes fitted**: each clip on its own scale, Absolute per
  lane from its head's menu.
- **The mouse frames an ordinary video in Framing only**: a stray click
  while editing moved it.

### The edit

- **Operations core**: every change to a project is a checked, undoable
  operation, open to the command line and to an AI agent (`omnix apply`,
  `omnix describe`, `omnix ops-schema`, `omnix looks`).
- **Selections and groups**: several cuts at once; groups that move
  together; sync lock, so sound, titles and pictures follow the picture
  they lie under; nothing runs past the end of the film.
- **Cuts on the beat**: a song's beats found and snapped to.
- **Markers**, a blank stretch, inserting between cuts, a stretch played
  from the ruler, the playhead moved by keys in sensible steps, every
  shortcut in one window.
- **Transitions**: a transitions lane and templates, edge looks (fade,
  blur, zoom, glitch, flash), each look's own settings.
- **Automation lanes**: effects, view, frame and overlay values as curves
  with keys, folding under each lane.
- **Framing an ordinary video**: a base frame, Frame lanes with keys and
  curves, a box in the viewer.

### Sound

- **Sound lanes**: music, two for effects, voice; each clip with its own
  volume line, fades and level; waveforms drawn as heard.
- **Voice**: a cleaned voice in place of a clip's sound; a voice-over
  recorded inside Omnix; a cut's voice chosen in the edit.
- **Loudness**: speech brought to −16 LUFS, normalize and even out,
  ducking under talk (also in a clip's own sound), a true-peak limit on
  export, a mix check against a platform's levels.

### Picture

- **Effects** in groups (and a wide view): black and white levels,
  vignette shapes, glow and blur, cinematic bars, auto colour from the
  picture, styles (old film, VHS, cyberpunk); scopes (histogram,
  waveform).
- **Text and pictures**: lanes in groups, dragging pictures in, styles,
  appear and disappear animations, kinds (sticker, polaroid,
  picture-in-picture).

### Around the edit

- **The bin** as a view of its own, quick copies of ordinary videos for
  smooth playback, and **Dailix**, the contact sheet before the edit:
  trips, days, marks, cleaned voices, projects made from the marking.
- **Project cards**: the start screen's edits as mosaics of their own
  frames, changing while the pointer is over them (`omnix card`).
- **Its own name and face**: `omnix`, its icon, its start screen.

### Under the hood

- From 19,000 to 50,000 lines of Rust; from 48 tests to 400, with
  snapshots of every time the code derives and CLI tests exporting real
  files through ffmpeg; CI that runs the GPU tests on a software
  renderer.
- The contact sheet's core (`crates/sheet`), shared with Dailix, in this
  repo's workspace.
