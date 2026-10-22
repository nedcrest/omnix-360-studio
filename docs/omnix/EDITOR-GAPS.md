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

### 2. A bin (project library panel), like Kdenlive's — v1 done

User: wants a bin, maybe with better UI, with "our extra sound tracks" in it.
**v1 (`src/player/bin.rs`, commit `241f28d`)**: B or the box icon in the top
bar opens it at the left. It lists the project's folder (above `04_project`,
footage-agent's `layout.py`): Footage, External video, Music, Voice-over,
Cleaned voice (`voice/<day>/<clip>/<recipe>.flac`), Sound effects, Graphics,
then the clips already loaded. A click adds: clip → cut at the end, sound →
the music lane at the playhead, picture → overlay at the playhead. Needs a
saved project (no folder otherwise).

Next for it, after the user tries v1: clip → at the playhead (item 1);
sounds → their own lanes (item 3); thumbnails / lengths; drag-and-drop
(harder in iced); footage-agent's ratings and notes from `report/notes.json`;
LUTs from `09_luts`.

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
- **Step B (large, later):** unlink sound from picture (J/L cuts: sound starts
  before the picture), and let a cleaned voice replace a cut's sound. Data model
  change; decide after A and 3 are in use.

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
