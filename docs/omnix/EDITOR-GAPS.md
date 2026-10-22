# Editor gaps — from the first real edit

The user's first test, 2026-10-22: the Kos 22.08 project made by
footage-agent's «→ Omnix» button opened, its clips are in order and play.
What they missed while editing, what the code already has, and how to fix
each. Not started; order below is the proposal.

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

### 2. A bin (project library panel), like Kdenlive's — v2 done, in test

User: wants a bin, maybe with better UI, with "our extra sound tracks" in it.

**Now (`src/player/bin.rs`, v1 `241f28d`, v2 `e33f65e`):**
- A panel of its own at the far left, beside whichever left panel (Info,
  Settings, Text) is open; folds to a narrow strip (B toggles).
- Pin (🔒) keeps it open, and open at the next start (`Settings.bin_pinned`).
- Lists the project's folder (above `04_project`, footage-agent's
  `layout.py`): Footage, External video, Music, Voice-over, Cleaned voice
  (`voice/<day>/<clip>/<recipe>.flac`), Sound effects, Graphics, then the
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
- preview inside the app's own viewer instead of an ffplay window.

### 3a. Cleaned voice in place of a clip's sound — NEXT (user, 2026-10-22)

The user's next ask: the cleaned voice chosen in the contact sheet
(`notes.json` → `clips["<day>/<file>"].audio = "<recipe>"`, file at
`voice/<day>/<clip stem>/<recipe>.flac`, same timing as the clip) should play
**instead of the clip's own sound**, and Omnix must read that from what
footage-agent exports. Plan: optional `Cut.sound: Option<PathBuf>` (a file
timed like the clip, replacing its sound; old projects unchanged);
playback (`audio.rs`) and export (ffmpeg graph) take it when set;
footage-agent's `export/omnix.py` writes it from `notes.json` (the Kdenlive
path already does this via `chosen_voices` in `cut.py`); the bin's Cleaned
voice gets "use for this cut". Then 3 (more lanes) for sfx and voice-over.

### 3. More sound lanes — medium-large, format change

Needed for sfx, voice-over and stinger sounds (`TRANSITIONS.md`).
`Project` gets a new optional field, e.g. `sounds: Vec<SoundLane>` (each a
name, level, clips like `MusicClip` placed at edit time, volume points). Old
projects load unchanged (`#[serde(default)]`). Export mixes them in the ffmpeg
graph beside music; playback mixes them in `audio.rs`. The music lane's
drawing and dragging code is the model to reuse.

### 4. The clip's sound as its own lane — two steps

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

## Format rule for all of these

New fields only, each `#[serde(default, skip_serializing_if = …)]`, so every
old project still opens (public format, upstream `AGENTS.md`). Note: an *older*
Omnix will refuse a project with the new fields (`deny_unknown_fields`) — fine,
we only run our own build.

## Proposed order

Taken by the user: 2 first (v1 done). Then 1 → 3 → 5 → 4A → 7 → 8; 6 any
time; 4B when the rest is in use. 1 is a few hours and helps at
once; 2 is the user's main ask and is where 3 and 5 plug in.
