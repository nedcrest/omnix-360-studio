# Editor gaps — from the first real edit

The user's first test, 2026-10-22: the Kos 22.08 project made by
footage-agent's «→ Omnix» button opened, its clips are in order and play.
What they missed while editing, what the code already has, and how to fix
each. Since then every new idea lands here too, numbered in arrival order.
The numbers are identities, not priorities; the order of work is the
milestones at the bottom (and `ROADMAP.md`'s `віха` field).

## What the timeline has today (code facts)

`src/player/timeline.rs` draws, top to bottom:

| Row | Holds | Data |
|---|---|---|
| Text lanes | titles | `Project.text: Vec<overlay::Lane>` |
| Overlay lanes | pictures over the picture | `Project.overlays: Vec<overlay::Lane>` |
| Cuts | the video, thumbnails, label `clip · length · note` | `Project.cuts` |
| Effects / View | keyframe diamonds of the cut | `Cut.effects`, `Cut.keyframes` |
| Seek bar | playhead | — |
| Music lane | **one** lane of songs with waveforms, shown once there's music | `Project.music.clips`, `music.keyframes` |

- A cut's own sound has **no lane**: its volume points (`Cut.volume`) are drawn
  as a line over the cut's thumbnails, added with Ctrl+click.
- Music clips never overlap except at a crossfade (`Music::fits`), so there is
  room for one sound at a time: no sfx under the music, no voice-over.
- Audio panel (`audio_panel.rs`): two levels only, clip sound and music.
- **Split exists**: C or the scissors splits the cut at the playhead
  (`edit.rs::split`). **Reorder exists**: drag a cut (`Drag::Move`).
- New clips always go **to the end** (`add_cut`, mod.rs ~2649), only via the
  file dialog (Ctrl+O). `App::library` holds loaded clips, but nothing shows it.
- No markers. footage-agent's marks arrive only as the cut's `note`;
  its flags («що знайти») don't arrive at all.

## Gaps and proposed fixes

### 1. Insert a clip between two cuts — small

User: no way to "split the video and put another clip in there".
Today it takes three steps: C at the point, add the clip (lands at the end),
drag it back. Fix: an **insert at the playhead**: if the playhead is inside a
cut, split it there and put the new cut between the halves; on a boundary,
just put it there. Used by the bin (item 2) and by Ctrl+O. No format change.

### 2. A bin (project library panel), like Kdenlive's — v2 done; add buttons user-tested 2026-10-23

User: wants a bin, maybe with better UI, with "our extra sound tracks" in it.

**Now (`src/player/bin.rs`, v1 `241f28d`, v2 `e33f65e`):**
- A panel of its own at the far left, beside whichever left panel (Info,
  Settings, Text) is open; folds to a narrow strip (B toggles).
- Pin (🔒) keeps it open, and open at the next start (`Settings.bin_pinned`).
- Lists the project's folder (above `04_project`, footage-agent's
  `layout.py`): Footage, External video, Music, Voice-over, Sound effects,
  Graphics (cleaned voice left out at the user's word: it belongs to its clip
  and comes in through the export, gap 3a), then the
  clips already loaded (Kos footage lives in the archive). Needs a saved
  project. Re-read on Refresh or when the project's folder changes.
- Each file: a small frame (clips, pictures), its length, and add buttons:
  clips and sounds have three — at the end, at the playhead (the cut there is
  split around the new one: `Edit::insert_at`), at the join nearest the
  playhead (`Edit::nearest_join`); pictures one, at the playhead. This also
  covers gap 1 for the bin.
- Click previews: sound via `ffplay -nodisp`; clip in a small ffplay window
  plus four frames in the bin's preview box. Same file again or ■ stops;
  quitting stops it.

User's asks on v1 (2026-10-22), all in v2: pin/lock so it doesn't close;
preview and pre-listen on click; pictures ("хоча б картинками без гіфу");
a panel of its own that folds to the side, not a stand-in for another.

**UI research** (user asked to look at the best): Final Cut Pro's browser
*skimmer* (move the pointer over a filmstrip to scrub, no click; E appends,
W inserts at the playhead without dragging), DaVinci Resolve's media pool
(thumbnail / list views, icon size, hover scrub shown in the viewer),
CapCut (thumbnail grid, "+", drag to the timeline).

**Next for the bin:**
- hover-scrub a clip's picture (Final Cut's skimmer) — a canvas per row;
- E / W from the bin: append / insert at the playhead (needs gap 1);
- grid view with bigger pictures, and a size slider;
- drag-and-drop to the timeline (harder in iced);
- sounds to their own lanes (gap 3); LUTs from `09_luts`;
- footage-agent's ratings, notes and tags from `report/notes.json`;
- **preview inside the app's own viewer instead of an ffplay window** — the
  user noticed the separate window (2026-10-22) and expects it in Omnix's
  viewer, like a source monitor in Kdenlive / Resolve. Idea: a preview mode
  that plays the bin's file in the main view (loaded into the library, not
  added to the edit), with its own in/out marks later (add only a part);
  Esc or another click returns to the edit at the playhead.

### 3a. Cleaned voice in place of a clip's sound — done, user-tested 2026-10-23

User's ask (2026-10-22). **Done** (`b9f1fc5` here, footage-agent `export/omnix.py`):
optional `Cut.audio`, a file timed like the clip, heard instead of its sound
in the player and the export; label "♪ <name>", Info row "Sound"; a missing
file falls back to the clip's sound with a warning. footage-agent writes it
from the contact sheet's choice (`notes.json` clips[].audio). Export now pads
each cut's sound to its frames' length (cleaned voice is ~30 ms short, camera
audio ~8 ms): measured, no drift between cuts. The bin leaves cleaned voice
out (user: it belongs to its clip).

Not yet: switching a cut's audio file inside Omnix (Info panel: "use the
clip's own" / pick a recipe). Wait for the user to ask.

### 3. Sound lanes — built and user-tested 2026-10-23 (with 4A)

**User's test, 2026-10-23:**
- the clip sound lane is there;
- music goes onto Music from its button;
- the bin's sound effects land on SFX and sound when the playhead
  reaches them, together with the video and the music;
- volume lines work on every lane.

Export not tried yet on a real project (big). The synthetic export was
measured, see below.

**User, 2026-10-23, after testing the bin and cleaned voice:** the music
lane has no name saying it is a sound lane for music. They want an **SFX
lane** for dropping in a sound effect, and the **clip's own sound as a lane
under the video**: "it'll be handier that way".

**Principle (user, 2026-10-23):** a fixed set of named lanes that is
simply enough, not tracks the user adds and groups at will. That set is
handier, easy to automate (each lane has a stable name in the project),
and easy to work in. A free-form mode can come later if ever needed.

**Built:**
- **Data and export** (`a5041b1`): `Project.sfx: Music`, `SoundTrack
  {Music, Sfx}`; the export mixes every track; Clips export writes
  `- sfx 001.wav`. Measured: SFX beeps land at 2.0002 s and 4.2502 s, at
  −11.97 dB against −12 expected.
- **Lanes** (`00e56ef`): top to bottom Text, Overlay, cuts, Clip sound, key
  rows, seek bar, Music, SFX. Every lane has a name tag pinned at the left
  edge with a dot of its colour. SFX clips are purple, the music green. All
  music interactions work on SFX: move, trim, Shift crossfade, Ctrl+click
  and K volume points, C split, Del, right-click Info. Edges snap across
  sound lanes. The Audio panel has an SFX level. Bin «Sound effects» goes to
  SFX; «Music» and «Voice-over» go to Music.
- **Clip sound lane** (`4db0023`), which is 4A: each cut's waveform (its
  cleaned voice, else the clip's sound from the `.LRF`) and its volume line,
  moved off the thumbnails. Read two files at a time, cached.

**Not yet:**
- a voice-over lane (one more `SoundTrack`);
- two sound effects at once (a second SFX lane);
- auto-duck;
- fade handles on a sound clip's corners.

### 4. The clip's sound as its own lane — 4A built with 3; 4B its own milestone

User: would split into video tracks and sound tracks.
- **Step A (small-medium):** draw a "Clip sound" lane under the cuts, with each
  cut's waveform and its volume line there (same data, `Cut.volume`), instead
  of over the thumbnails. Still moves with its cut.
- **Step B (large): group / ungroup a cut's picture and sound** (user,
  2026-10-22). Ungrouped, each moves and trims on its own. The user's case:
  someone talks; keep their sound running and lay **B-roll picture over it**
  (a cutaway), then move the pieces separately. Also J/L cuts (sound starts
  before the picture) and a cleaned voice replacing a cut's sound. Data model
  change: a cut's sound becomes an item of its own on a sound lane, linked to
  its picture by a group id; regrouping links them again.
  A cheaper route to the cutaway alone: **video on the overlay lane** (today
  overlays are pictures and text only) — a B-roll clip over the talking cut,
  muted, the talk's sound untouched. Worth weighing first: smaller, no
  unlinking. Raised in priority by this ask.
- **User, more precisely (2026-10-23):** "ungroup" means that a cut's
  picture and its own sound can be trimmed to different lengths:
  - shorten the video and keep the sound longer;
  - or shorten the sound and keep the video longer.

  This leaves room to add music or a voice-over at the right spot, or to put
  a picture on the overlay lane while the talk goes on. Their example: "we
  were by the castle but didn't go in; I hope I don't forget to show you the
  photo", with the photo or a GIF on the overlay lane meanwhile. The overlay
  lane already does the photo part.
- **Design note: a split edit first, inside the magnetic edit.** This is
  Final Cut's "expand audio" / J- and L-cuts, and it is cheaper than a full
  ungroup:
  - Each cut gets optional sound extensions, seconds before its in point and
    after its out point (new optional fields, so old projects are
    unchanged).
  - On the Clip sound lane, the cut's sound block is drawn wider or narrower
    than its picture. Dragging its edge there trims only the sound.
  - Because the edit is magnetic, the next cut still starts where this
    picture ends. A sound running longer plays on under the next cut's start
    (an L-cut); the two sounds overlap, each at its level. A sound shorter
    than its picture leaves silence for music or voice-over.
  - Export: each cut's sound stretch is cut by its own in/out and placed at
    its edit time in the mix, instead of joined end to end. The player
    needs the next cut's sound primed early (it already pre-rolls 3 s).
  - The full ungroup (independent items, group ids) can come later, if this
    falls short.
- **Connected clips** (from `TIMELINE-UX.md`, FCP / CapCut track linkage):
  overlays and voice-over attached to a cut, moving with it. Same data
  model as step B (a link between items), so it is designed with it and
  with 8 (multi-select), as one format change.

### 5. Markers — small-medium, format change

A marker is a time and a label. Pinned to clip time like keyframes, so it
moves with its cut: new optional `Cut.marks: Vec<{t, note}>`, drawn as flags
on the seek bar; M adds one at the playhead, click jumps. footage-agent then
exports its flags («що знайти») and point marks into them
(`footage_agent/export/omnix.py`).

### 6. Vignette shapes — idea (user, 2026-10-22)

Today there is one vignette: `Grade.vignette` (0..1, "Darker corners"),
round, by distance from the centre (`grade.rs` ~136, mirrored in
`grade.wgsl`). The user wants **types**, e.g. on a sunny day with a deep
blue sky, darken only the **top edge** of the frame — a graduated filter
like an ND grad on a lens.

Possible shapes: round (today), top, bottom, left/right, and maybe an
angle. Parameters: amount (exists), shape, softness, how far in it reaches.
Keyframed like every effect, so a top grad can follow a tilt. Format: new
optional fields on the effect key; CPU (`grade.rs`) and GPU (`grade.wgsl`)
together with a GPU-vs-CPU test, as upstream `AGENTS.md` requires.
Small-medium; independent of the rest.

### 7. A wider selection frame — small (user, 2026-10-22)

The selected cut's frame on the timeline is hard to see. Make it thicker
and in the accent colour (`timeline.rs`, where the cut blocks are drawn).

### 8. Select several cuts, act on them together — medium (user, 2026-10-22)

Today one cut is selected (`App::selected: Option<usize>`). Wanted:
Shift/Ctrl+click to select several, then act on all — delete, move, set
effects, and **join** them (e.g. merge cuts of one clip back into one, or
group them so they move together). Needs a selection set instead of one
index, and a choice of what "join" means: merging neighbours from the same
clip is clear; a group of different clips is a format change (a group id).

### 9. Timeline ease and accessibility — research done (user, 2026-10-23)

The user asked for "maximum accessibility and convenience" after looking at
good editors. The research, the measured weak spots and an order of
eight packs are in `TIMELINE-UX.md`. Item 7 (selection frame) is now part of
its pack 1, and the bin's skimmer is shared with the timeline's. Keys that
clash with the 360 view (J/K/L, ↑/↓, E/Q, S) wait for the user's choice.

### 10. Two modes: Edit and Framing — built 2026-10-24 (`562dad6`), waiting for the user's test

The user's answer to the key clashes in `TIMELINE-UX.md`: Omnix is a video
editor first. By default it is in **Edit** mode, where keys and the mouse
edit the timeline. The timeline holds MP4s and raw Osmo 360 clips alike.
Selecting a 360 clip lights up a switch (a slider-style toggle) to **360
framing** mode, which has today's tools: looking around, view keyframes,
stabilisation, projection, follow and colour track.

Why it is right: every clash goes away without chords (J/K/L, ↑/↓, E/Q, S
mean editing in one mode and the view in the other). The editing screen
loses the 360-only controls. A stray drag can no longer reframe a cut,
which matters with Auto keyframe on.

To settle when designing it:
- **What Edit mode shows of a 360 cut.** Its keyframed view, played as
  exported. Does dragging the picture do nothing, or temporarily look
  around without keying?
- **Ordinary videos.** They have pan/zoom framing too (`view::Crop`). Should
  the switch also light up for them, as «Кадрування»? Proposal: yes, with the
  360-only parts hidden.
- **Leaving the mode.** Esc, the switch, or selecting a non-360 cut. Moving to
  another 360 cut keeps the mode.
- **Showing the mode.** A coloured frame around the viewer plus the word
  "360" in the switch, never colour alone (WCAG 1.4.1).
- **Where 360-only controls go.** Stab/Projection pickers, the angles strip
  and the View keyframe row show only in 360 mode.
- **Cost.** Key handling in `mod.rs` splits by mode, which makes merges from
  upstream harder there.

**Layout review (user, 2026-10-23: "much of the buttons is aimed at 360;
worth redoing? what would you change now, putting the 360 part away until
we get to it?").** What shows today, read from `view()` in `mod.rs`:
- **Top bar:** ⚙, New, +Clip, +Music, +Overlay, +Text, Open, Save, Save as,
  Aspect / Resolution / Export…; then for a 360 cut Stabilisation,
  Projection, Source, Full res; meter, fullscreen, the proxy and GPU status
  icons. File and Add buttons are interleaved.
- **Transport row:** Play, Save view PNG, Save sphere PNG, time; K, delete
  key, Ease, Follow, colour-track picker, FOV, Reset view; Effects,
  Crossfade, Split, Delete, Undo, Redo; four zoom controls; Audio. About a
  third of it is framing.
- **Angles strip** under it (on by default), and the timeline's **View**
  key row.

Proposal: **don't comment the code out.** That would be dead code against
upstream's rules, and would make every merge from upstream conflict.
**Hide it behind the mode instead:** gap 10 as the way to put it away,
one condition per group.
- **Edit mode (default):**
  - Top bar, grouped: ⚙ | New, Open, Save, Save as | +Clip, +Music,
    +Sound effect, +Overlay, +Text (+Blank, gap 32) | Aspect, Export… |
    meter, fullscreen.
  - Transport row: Play, time | Split, Delete, Crossfade | Undo, Redo |
    Effects, Audio | the «Framing» switch | timeline zoom.
  - "Save frame as PNG" stays: a still for a thumbnail is useful in any
    edit.
  - The timeline loses the View key row (20 px more for the lanes).
  - The 360 keys are off: arrows, Q/E, P, S, 0.
- **Framing mode** adds, for the current cut:
  - K, delete key, Ease, FOV, Reset view, and the angles strip;
  - the View key row;
  - for a 360 cut also Follow, the colour track, Save sphere, and
    Stabilisation, Projection, Source and Full res in the top bar;
  - the GPU and proxy status icons (they matter for 360 decoding).
- Keyframes already in a project keep playing and exporting in Edit mode.
  Only editing them needs Framing mode. Dragging the picture in Edit mode
  does nothing.
- Should the switch also light up for ordinary videos and pictures? A
  push-in over a photo (Ken Burns) is the same pan/zoom keyframes. Built
  as proposed, not yet confirmed by the user: yes, with the 360-only parts
  hidden.
- **User's test, 2026-10-24:**
  - The groups are clear (the tooltips explain the rest).
  - Editing can't shift the view.
  - «+SFX» opens a file dialog and adds the sound at the playhead on SFX.
  - 360 is untested: no 360 clip in the project yet.
  - **Found:** framing one video shifted all the others. The shared free
    look showed on every ordinary video without keyframes, and a save
    wrote it into all of them. Fixed in a separate commit: unkeyed ordinary
    videos show their whole picture as the export does; only the cut being
    framed gets the look on save. None of the Kos saves had been affected.
- **As built:** `App::framing` (off each start), `framing_only(message)`
  and `framing_key(key)` drop what turns or keys the view while editing.
  K on a selected sound still adds a volume point; Del deletes the selected
  cut. `Rows::new(scale, framing)` leaves out the View row. The bars are
  grouped with `separator()`. New: «+SFX» (`AddSound(Sfx)`), a sound effect
  at the playhead.
- Separate, later: the window title and the credit line say «rOsmo360».
  Branding the fork as Omnix is its own small decision.

### 11. Bring a speech clip up to −16 LUFS — small-medium (Kos TODO, 2026-10-23)

From `den1-TODO.md` (measured on the Kos day-1 master, made in Kdenlive):
integrated −24.4 LUFS, LRA 17.3 LU, true peak 0.0 dBFS. The film is 8 dB
quieter than the web's ~−16, and lifting the whole mix would clip. The
TODO's fix is to normalise **each speech clip on its own**. A button on a
cut (Audio panel or Info) measures its sound (`ffmpeg ebur128`, on the
cleaned voice if set) and sets a gain to −16 LUFS, stored as a new optional
cut level (format change) or as volume points (none). Measure both on Kos.
`speechnorm`, the TODO's extra lift for a far-away voice, is the
footage-agent cleaner's job, not Omnix's.

**Measured on Kos 22.08 DJI_0001 (2026-10-23)**, after the user said it
sounds "a bit quiet" at 0 dB:

| File | Integrated loudness | LRA | True peak |
|---|---|---|---|
| clip | −30.7 LUFS | 13.7 LU | −6.8 dBFS |
| cleaned voice `dnn6` | −32.2 LUFS | 16.0 LU | −10.5 dBFS |

So the clip is ~15 dB under −16 LUFS, and cleaning made it 1.5 dB quieter
still. The slider's 0 dB means "as recorded", not "loud enough". Findings
for the build:
- **The level sliders and volume points stop at +6 dB**, which is not
  enough for footage like this. Allow more, e.g. +24 dB, for the gain.
- **A gain this big needs a limiter.** +15 dB on a −6.8 dBFS peak goes 8 dB
  over full scale. Gain and 12 belong together: do 11 and 12 as one step.
- **The LRA (13–16 LU) is wide for speech.** Quiet words stay quiet next to
  loud ones even at the right average. An optional gentle compressor (or
  `speechnorm` in footage-agent's cleaner) would even it out. Decide after
  hearing the plain gain plus limiter.

**The user's design (2026-10-23): «Normalize video sound»**, "so I don't
have to fiddle with levels myself":
- Scope: **all cuts** or **the selected cut**.
- Fixed, built-in settings: bring the average up to a target, **lift the
  quiet parts and tame the loud ones**. No knobs to learn.

How to build it so the player and the export sound the same:
- **Step 1, a measured gain per cut.** Measure the stretch the cut plays
  (`ebur128` on its own in–out, on the cleaned voice where chosen) and
  store the gain to −16 LUFS as a new optional cut field (shown in Info,
  undoable, one step for "all"). The player and the export apply the same
  number, and the volume line still works on top. «All cuts» measures each
  cut on a thread with progress; a file is decoded once per cut stretch
  (seconds each).
- **Step 1 ships with gap 12's limiter** at the end of the export mix
  (−1 dBTP). Without it a +15 dB gain clips.
- **Step 2, «lift quiet, tame loud».** A fixed, gentle speech leveler per
  cut (ffmpeg `speechnorm` or `dynaudnorm` with preset values, chosen by ear
  on Kos). It sits in the player's audio decode and in the export's stretch
  chain, the same filter in both. After a seek mid-cut its first second can
  differ slightly from the export, which starts at the in point. Tried on
  DJI_0001 against the plain gain, by ear, before deciding the preset.
- The music is levelled separately (its own target under speech, then
  auto-duck). This is about the clips' own sound only.
- **A dialog with progress** (user, 2026-10-23). The button opens a modal
  window: what it will do, the scope (all cuts or the selected one), a
  «Start» button. After «Start» a progress bar runs ("cut 12 of 39"), with
  Cancel. When it's done it says what changed (e.g. "39 cuts, +9 to +17
  dB") and closes. Today long work (export, colour tracking) shows its
  progress and Cancel in the top bar. A modal is new in Omnix: iced draws it
  as a `stack` layer over the window with the rest dimmed. Built once, for
  any long job later (transcripts, waveforms of a whole project).

### 12. True-peak limit on export — small (Kos TODO, 2026-10-23)

The export's mix ends in `amix` followed only by the edit's fades (`ffmpeg.rs` ~1027). Add a limiter for
−1 dBTP (`alimiter` or `loudnorm` with TP=-1 in the final stage). Check with
`ebur128=peak=true` on an export that clips today. No format change; maybe
a setting later.

### 13. Cuts on the beat — medium (Kos TODO, 2026-10-23)

The TODO: "a cut doesn't land on the beat", found by ear. Find the music's
beats (onset or beat tracking: ffmpeg has no beat tracker, so a small Rust
onset detector or footage-agent's Python with librosa or aubio), list the
cuts more than N ms off a beat (`rosmo360 check` gains a `beat` rule), and
draw the beats as marks on the music lane that cuts snap to (needs 5 and
the snapping pack of 9).

### 14. Black and white levels — small (Kos TODO, 2026-10-23)

The Kos master's darkest pixel is 14 of 255 (median over 1968 frames):
nothing is black, so it looks flat. The TODO's fix is levels: black input
≈ 0.055, white ≈ 0.95, then a little saturation. `Grade` has exposure,
contrast and highlights/shadows, but no black and white points. Add `black_in`
and `white_in` to the effect key, mirrored in `grade.rs` and `grade.wgsl`
with a GPU-vs-CPU test. "To all cuts" already applies a look to the whole
film. The name `black` is taken by the fades, so pick another.

### 15. Cinematic bars — small (Kos TODO, 2026-10-23)

The TODO's plan: on 30 s of forest B-roll, black bars slide in from top and
bottom (2.39:1, 194 px each at 2688×1512), a colour shift and a vignette,
then back. Possible today by hand: two black PNGs on the overlay lane with
motion keys. Proper: an effect value "bars" (0 = none, 1 = 2.39:1), keyframed,
so it eases in and out per cut. Small with 14's plumbing.

### 16. Subtitles from speech — medium (Kos TODO, 2026-10-23)

Mar'yana can't be heard even cleaned (action-cam mic, distance). Run
Whisper only on the cuts where someone speaks (the TODO warns CPU Whisper is
slow), on the cleaned voice. Put the draft as items on the text lane, one
per phrase, timed, to be fixed by hand. Recognition belongs in footage-agent
(Python, it already has the voices). Omnix gets the result through
`export/omnix.py` as `text` lane items (existing format) or imports an `.srt`
(new). The same transcript later feeds text-based editing (9).

### 17–24. From OpenOSV — research done (user, 2026-10-23)

The user asked what OpenOSV (Apache-2.0, a C++/CUDA OSV plug-in for
Premiere/Resolve) offers us. Full findings, sources and the licence notes are
in `OPENOSV.md`. Short list:
- **17. Seven easing presets** (DJI Studio's names): small, works for every
  keyframe now. Format: new `Ease` variants, old projects unchanged.
- **18. Bundled D-Log M → Rec.709 LUT** (their generated, DJI-free `.cube`,
  resampled to 33³, NOTICE attribution): small.
- **19. Lens-protector correction** from `StreamMeta.7 extri_lens_mode`: small–medium.
- **20. Release the mount mask where both lenses agree** (their Hide Mount
  Auto): aimed at the 0025 cap brim. CPU prototype first.
- **21. Per-clip lens tilt.** Diagnostic first (fit tilt to our seam-shift
  medians); port the rotation fit only if a residual shows.
- **22. Sky seam field** from trusted pixels plus a usable-rim measure; check
  for a sun-side ring. ⚠ GoPro flare patents: check first.
- **23. DJI framing presets** (Crystal Ball, Asteroid, Wide, Ultra Wide,
  Dewarping) and DJI's coupled zoom (FOV and correction together).
- **24. File facts:**
  - LRF timing on clips with dropped frames;
  - the 6K scale (0.794492) and the 1920 px scale check;
  - ISO, shutter, WB and readout time into the Info panel.

### 25. Transitions and stingers — spec in `TRANSITIONS.md` (user, 2026-10-22/23)

Split into steps so each can be tried on Kos on its own:
- **25a. Basic transitions and quick placement:** dissolve with a curve,
  dip to colour, wipe, zoom blur. Three ways to add one, in
  `TRANSITIONS.md` → "Quick ways": T at the playhead, between two selected
  clips, or on every join. Builds on the existing crossfade.
- **25b. Edge transitions** (out on one cut, in on the next, no overlap).
- **25c. Templates:** a configured transition saved into the project and
  picked for any join.
- **25d. Stingers («перебивки», video transitions)**, their own yellow
  group: a short clip with its own sound on a cut, insert or cover, from the
  project's `06_external` / `07_sfx`. Needs the overlay lane to take video
  (milestone 4's first card) and sound lanes (milestone 1).
- **25e. 360 looks:** whip pan, little-planet zoom, sphere roll.

### 26. Glow and blur — small-medium (Kos TODO, 2026-10-23)

The Kos TODO's «Міст» shot wants Kdenlive's "Blur of the glow" (blur 0.2,
brightness 0.1) to soften hard edges. A keyframed effect value: a blur
radius plus a glow (the blurred highlights added back). CPU and GPU with a
parity test. Blur is a multi-tap pass, so it costs more than a per-pixel
effect.

### 27. Overview strip and centred playhead — later (`TIMELINE-UX.md` pack 8)

A thin strip of the whole edit above the lanes while zoomed in (Resolve's
dual timeline, cheaply), and a centred playhead as an option (CapCut,
iMovie). Both are nice to have, not urgent.

### 28. AI auto-edit from the contact sheet — the user's main ask (2026-10-23)

"The main thing I'd like": an API, a skill and a snippet so an AI (Claude or
another) can build the edit from the contact sheet's notes and marks as its
brief, by rules written down properly. Realistic for a good first edit that
the user polishes. The brief already exists (notes, roles, day stories), and
so do the tools (`check`, `preview`, `export/omnix.py`). Design and steps
are in `AUTO-EDIT.md`:
- 28a brief export (footage-agent);
- 28b skill v0 with the rules, the snippet and the loop, tried on Kos 22.08;
- 28c Omnix `schema`, `describe` and reloading the project when its file
  changes;
- 28d brief fields in the contact sheet;
- 28e transcripts (from gap 29);
- 28f the review loop.

### 29. Speech transcription — medium (user, 2026-10-23)

footage-agent's planned `speech.py` (README: ⬜, the `speech` Poetry group
already lists faster-whisper; not installed). Whisper locally on the RTX
3060 (large-v3 fits 12 GB; Ukrainian is supported), word timestamps, run on
the cleaned voice where one is chosen, else the clip's sound, and only
where there is speech. Written to the clip's sidecar. It feeds subtitles
(16), text-based editing, search by what was said, the bin and Info panel,
and later AI auto-edit cutting on phrases (28). Licence: faster-whisper
(MIT), Whisper weights (MIT).

### 30. Who is in the frame — medium (user, 2026-10-23)

footage-agent's planned `faces.py` (README: ⬜, the `faces` group lists
insightface + onnxruntime-gpu). The README's plan: take the named people's
reference embeddings from the home Immich, which already recognises and
names them with the same model family (InsightFace / ArcFace), so nothing is
labelled twice. Per clip: who appears, when, how big. Omnix shows it in the
bin (filter by person) and Info.
- ⚠ **InsightFace's pretrained models are for non-commercial research
  only.** That is fine for home use, but not shippable in a published
  Omnix. If this ever goes public, use OpenCV's YuNet (detection) and SFace
  (recognition), both Apache-2.0. Immich's own models are the same family,
  so their licence applies to the embeddings we read too.
- Also check Immich's API for reading faces (read-only, the user's own
  server).

### 31. Speed: slow motion and fast forward, with automation — large (user, 2026-10-23)

User: "Missing: slowing video down (and speeding it up). I don't know yet how
best to do it. I have FL Studio experience with automating synth and
instrument parameters, and I see we have an «Effects» row: maybe group video
effects and sound effects (but for a specific track). For speed, the clip's
native sound has to slow down or speed up with it, so the timeline doesn't
drift: a big, careful layer of work."

What the code already gives:
- Frames are picked by time, not by count (0.7.1's mixed-frame-rate fix),
  so a cut that reads its clip slower is natural. A 100 fps Osmo clip at
  0.3× is real slow motion; 30 fps at 0.5× repeats frames, or blends them
  later.
- Effect keyframes already ease values over a cut's time, which is FL's
  automation idea. A speed curve would be one more automated value.

To work out before building:
- **Constant speed per cut first** (`Cut.speed`, 0.25–4×, new optional
  field). Then a cut's length on the edit = (out − in) / speed. Every place
  that maps edit time ↔ clip time must use it:
  - `project::offsets`, `Edit::locate`;
  - keyframes, effects and volume points (all stored in clip time, so they
    stay on their footage);
  - crossfades, thumbnails, the player clock, both exports and `check`.
- **Sound follows the speed.** ffmpeg `atempo` keeps the pitch (chained for
  < 0.5×) and is constant only; `asetrate` shifts the pitch like tape. The
  player needs the same: decode through the same filter, so the playback
  clock matches. Pad to the frames' length as now (`pad`), so later cuts
  don't drift. Measure the sync by cross-correlation as for cleaned voice.
- **Speed ramps** (automation over time) second. Time becomes the integral
  of the speed curve. Sound must be done in pieces of constant tempo, or
  with a time-stretcher (`rubberband`, rarely in ffmpeg builds).
- **Grouped effects per track** (the FL idea): a cut's effects row could
  show lanes per automated value (exposure, speed, volume) that fold open,
  instead of one row of marks. A UI question, together with gap 9's action
  bar and the effects panel.

### 32. A blank stretch on the main lane — idea (user, 2026-10-23)

A button that puts a blank (black) cut of a chosen length at the playhead
or at a join: room for an overlay, a title or a sound with a custom stinger
over it. The user notes it can be done today by adding a picture or a GIF as
a cut. Cheapest build: a generated black picture (pictures already work as
cuts, 5 s by default), named "Blank" on the timeline, and maybe a colour
choice. Keeps the edit magnetic: the blank is a cut like any other.

The user also confirmed (2026-10-23): **the magnetic edit is handy.** Keep
it the default; gaps happen only as blank cuts.

### 33. Right-click menu on a cut's sound — small-medium (user, 2026-10-24)

Right-click on the Clip sound lane offers:
- **«Lower for voice-over»** to a set level. Proposed: −12 dB under the
  normalized level, the common duck depth. The menu could show −6 / −12 /
  −18 with −12 first.
- **«Mute»**, and «Unmute» when muted.
- **«Normalize…»** opens the normalize modal (gap 11) scoped to this cut;
  more settings may come there later.
- «Info» stays in the menu: right-click opens the Info panel today.

How it fits:
- All four act on **one per-cut level**, the field gap 11 adds for the
  measured gain. Normalize sets it, «lower» subtracts 12 dB from it, mute
  sets it to off (`OFF_DB`). One number, shown in Info, applied the same
  in the player and the export, undoable. The volume line still works on
  top.
- Muted and lowered cuts should show it on the lane: a dimmed waveform
  with a «−12» or «muted» tag, never colour alone.
- iced has no native context menu. Build a small menu as a layer at the
  pointer, closed by Esc or a click elsewhere. Make it once, for every
  lane; music and SFX clips want the same menu later. Context menus at the
  pointer are one of the Fitts's-law points in `TIMELINE-UX.md`.

### 34. Import from devices in the studio — medium, footage-agent (user, 2026-10-28)

The user recalled wanting a new project plus dropping files into it from
devices (phone, camera, drone), "or some handy integration". Then they
saw part of it was done. What exists and what doesn't:
- **Exists:** the studio's «Новий проєкт» (`95d646f`: creates the folder
  tree) and `scripts/ingest.py` (CLI). It finds mounted volumes with a
  `DCIM` folder, takes the day's files (`--date`), merges the camera's
  internal memory and its card, puts `.LRF` aside as proxies, never deletes
  originals, and has `--dry-run`.
- **Missing:**
  - an **«Імпорт» button in the studio** for the chosen project;
  - a **phone** path: Android mounts over MTP under
    `/run/user/1000/gvfs/mtp:*`, not `/media`, so `find_sources` misses it;
  - checking the **drone** (DJI Neo 2 card) and Osmo 360 cards:
    probably DCIM too, check;
  - **skipping files already imported** (size and name, or a short hash),
    so the same card can be read twice;
  - a **per-device subfolder** in `01_footage` (`phone/`, `action2/`,
    `neo2/`, `osmo360/`), from the camera's metadata or file name;
  - **progress** while copying (a long job, like gap 11's modal).
- Footage-agent's side: the other agent may take it. Recorded in
  `docs/OMNIX.md` there too.

### 35. Code analysis and refactor: scalable, API-driven, documented (user, 2026-10-28)

"After the patch, before the check: an analysis of the code (I don't
remember if we did one), a refactor to make it easily scalable and
API-driven: not stuffing everything into one script or one class, but
separating it properly, and documenting it too."

What we know already:
- Upstream had a code audit once (0.6.0, seven fixes; it noted
  `player/mod.rs` was only skimmed). We never did one on our changes.
- `src/player/mod.rs` is over 4000 lines: `App` holds the state, every
  message's handling, the keys and the whole `view()`. `src/main.rs` is
  the CLI plus the export pipeline. This is the "one class" the user means.

Plan:
- **Analysis first**, written to `docs/omnix/ARCHITECTURE.md`. It covers
  the modules and what each owns, where state and logic tangle, and what an
  "API" means here: a plain-data edit model with operations (`edit.rs`
  already is one) that the UI, the CLI and later an AI all call. Also
  risks, bugs found on the way, and an order of steps.
- **Then the refactor in small, behaviour-free commits**, with tests after
  each and the measured paths (export timing, loudness) re-checked:
  - split `App::handle` by area (sound tracks, overlays, keyframes,
    project file, bin) into modules;
  - move the edit operations the UI does inline (sound clips' move, trim
    and split, normalize) into `edit.rs` / `project.rs` with tests;
  - put the bars' view code next to its messages.
- **The cost to name:** upstream's `player/mod.rs` changes will no longer
  merge cleanly. That is acceptable for a fork that has become its own
  editor, but from then on taking upstream changes means porting them by
  hand. The analysis says which upstream areas (stitch, colour, export
  pipeline) we keep untouched so they still merge.

## Format rule for all of these

New fields only, each `#[serde(default, skip_serializing_if = …)]`, so every
old project still opens (public format, upstream `AGENTS.md`). Note: an *older*
Omnix will refuse a project with the new fields (`deny_unknown_fields`) — fine,
we only run our own build.

## Milestones — the order of work (agreed with the user, 2026-10-23)

The user throws ideas in as they come; they are sorted here and regrouped
whenever a better grouping shows. `ROADMAP.md` shows the same as cards (field
`віха`) with each milestone's progress.

1. **Звук для Косу** (sound): 33 the right-click menu goes with 11 (both
   use the per-cut level). 3a cleaned voice (done) → 3 + 4A sound
   lanes, designed together → 11 per-clip −16 LUFS → auto-duck (in 3) →
   12 true-peak limit. First, because the Kos film's worst problem is sound,
   and its TODO puts sound right after the cuts.
2. **Зручний таймлайн** (timeline ease, runs beside 1 in small packs): 10
   two modes → 9 pack 1 (with 7) → 5 markers → 9 click routes → 9 snapping
   → 9 skimmer (needs 10 for S) → 9 action bar → 9 typed time and UI scale
   → 27 overview strip, centred playhead option.
3. **Матеріал і бін** (material in): 34 import from devices in the studio; 1 + 2 bin buttons (done) → bin
   preview in Omnix's viewer (designed with 10: the viewer gets three
   states) → bin grid, ratings from `notes.json`, LUTs, E/W → «new project
   with folders» in Omnix.
4. **Розуміння матеріалу** (footage-agent, can run beside the Omnix work):
   29 speech transcription → 30 who is in the frame (names from Immich).
   Feeds subtitles, text editing, bin search, and later AI auto-edit.
5. **B-roll і групи**: video on the overlay lane (cheap cutaway) → 4B
   group/ungroup + connected clips + 8 multi-select, one format change,
   designed as one.
6. **Ритм і переходи**: 13 beats → 25a basic transitions with quick
   placement → 25d stingers («перебивки») → 25c templates → 25b edge
   transitions → 25e 360 looks. Stingers come early because the user
   already has stinger clips (film burn, white noise).
7. **Колір і текст**: 14 levels → 15 bars → 26 glow/blur → 6 vignette shapes →
   16 subtitles (needs 29) → text-based editing (needs 29).
8. **Швидкість і автоматизація** (gap 31): constant speed per cut with
   its sound following (atempo), measured for drift → speed ramps →
   automation lanes per value (the user's FL Studio idea). Large and
   careful; after B-roll and groups, since it touches the same time
   mapping.
9. **Якість 360** (from `OPENOSV.md`, mostly needs 360 footage: one public
   sample now, the user's camera ~2027-02): 17 easing presets and 18 bundled
   LUT any time (small, useful now) → 24 file facts → 19 lens protectors →
   20 mask release → 21 tilt diagnostic → 22 sky seam field (patents first)
   → 23 DJI presets → Osmo 360 II.
10. **ШІ-монтаж** (gap 28, `AUTO-EDIT.md`): the user's main wish, but put
   last on purpose (user, 2026-10-23: "I wouldn't do AI functions yet:
   Omnix doesn't have its full working functionality"). The design stays
   ready. The pieces it needs arrive with the milestones above.
