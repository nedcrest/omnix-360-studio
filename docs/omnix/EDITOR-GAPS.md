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

### 2. A bin (project library panel), like Kdenlive's — medium

User: wants a bin, maybe with better UI, with "our extra sound tracks" in it.
A left panel (beside Settings / Info / Text) listing the **project folder**
— the parent of `04_project`, layout from footage-agent's
`footage_agent/layout.py`:

- Footage (`01_footage`, or the clips already in the edit / archive),
- Music (`02_audio/music`), Voice-over (`02_audio/voiceover`),
- Cleaned voice (`voice/<day>/<clip>/<recipe>.flac`, from the contact sheet),
- SFX (`07_sfx`), External video (`06_external`), Graphics (`08_graphics`),
  LUTs (`09_luts`).

Click (or Enter) adds at the playhead into the right place: video → cut
(item 1), music → music lane, sfx / voice → a sound lane (item 3), picture →
overlay lane. Drag-and-drop later (harder in iced). Later still: show
footage-agent's ratings and notes from `report/notes.json`.

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

## Format rule for all of these

New fields only, each `#[serde(default, skip_serializing_if = …)]`, so every
old project still opens (public format, upstream `AGENTS.md`). Note: an *older*
Omnix will refuse a project with the new fields (`deny_unknown_fields`) — fine,
we only run our own build.

## Proposed order

1 → 2 → 3 → 5 → 4A; 4B when the rest is in use. 1 is a few hours and helps at
once; 2 is the user's main ask and is where 3 and 5 plug in.
