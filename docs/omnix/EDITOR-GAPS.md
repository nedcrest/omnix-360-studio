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

**Built 2026-10-29:** the Voice lane (`a6776ec`) and fade handles on sound
clips' top corners (`bc19aab`).

**Auto-duck, built 2026-11-01** (`684f7d2`, `0efe6b4`), waiting for the
check. The user's design: one action and a depth; speech from the cleaned
voice; ordinary volume points. Defaults taken:
- speech is looked for in cuts with a cleaned voice and in Voice takes
  only; a cut's own sound without a cleaned voice doesn't count (wind and
  traffic as often as talk);
- found as peaks above the stretch's loudest less 24 dB (never under
  −50 dBFS), pauses under 0.8 s bridged, blips under 0.25 s dropped;
- the music goes down 0.3 s before speech and back up over 0.6 s after;
- depth 12 dB by default (3–30), remembered;
- the dip is added to the music's own line, so hand-drawn points keep
  their shape; running it again dips again (Ctrl+Z first to change the
  depth);
- an action, not a live switch: the points are the user's to edit after.
In the Audio panel («Lower under speech» and the depth), the Music lane's
menu, and `rosmo360 duck project.json [--depth] [-o]`. On the test
project: speech 5.7–11.7 s and 12.9–25.9 s, the music 12 dB down there.

**Not yet:** two sound effects at once (a second SFX lane).

### 4. The clip's sound as its own lane — 4A built with 3; 4B (J/L cuts) done 2026-11-03, confirmed by the user

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
  - **No full ungroup** (user, 2026-11-03): a sound can be pulled longer
    from its own clip over the neighbour, and voice-over and music have
    lanes of their own; independent sound items are not wanted.
- **Best practices (looked up 2026-11-03):** Final Cut, magnetic like us,
  keeps picture and sound linked and in sync: Expand Audio (Ctrl+S or a
  double-click on the waveform) shows the sound's own edges, and dragging
  one extends the sound over the neighbour; Align Audio to Video puts them
  back; the clips need media beyond their in/out (handles). Premiere:
  Alt-drag an edge (or Alt with the rolling edit) trims only the sound or
  only the picture of a linked clip, no unlinking. Resolve unlinks with
  the chain icon. Common to all: the split is a trim of the sound's edge
  that keeps sync (never a slide), the overlapping sounds both play, and
  a reset puts the sound back to the picture's edges. So for Omnix: drag
  a cut's sound edge on the Clip sound lane (Alt-drag on the cut's edge
  too, Premiere's way), short fades at the moved edges as the cut's sound
  fades already do, «Sound to picture» in the sound menu to reset, the
  edge stopping (red) at the clip's end. Later maybe a rolling split
  (one drag lengthens one sound and shortens the next).
- **Done 2026-11-03** (`71f1ef3` model and ops, `6ff67b4` export,
  `4931abe` player), confirmed by the user with the snapping:
  - `Cut::sound_before` / `sound_after` (seconds past the picture's edge,
    negative = short of it), `CutOp::SoundEdge { cut, end, t, most }` and
    `CutOp::SoundToPicture`. Trims carry them with the picture's edges
    (kept within the clip); a split keeps each at its own outer end.
  - The sound past the picture is played and mixed as sound clips
    (`Cut::sound_beyond`, `Project::sound_beyond`: one to a track at the
    clips' level, so a tail and the next head both play rather than
    crossfade); under the picture, what stops short is silent
    (`Cut::sound_fade`, `Sound::of_cut`'s `head`/`tail`). Fades sit at
    the sound's own edges.
  - Timeline: the cut's sound drawn over its own stretch, framed when
    split; the selected cut's sound edges dragged on the Clip sound lane
    (yellow / red handles, a readout); the sound menu's «Sound back to the
    picture's edges». The shortcuts window lists the drag.
  - The edge snaps (user, 2026-11-03: lining it up by hand was fiddly)
    within 12 px (every edge's snap, raised from 8 at the user's word) to the pictures' joins, other cuts' sound edges, sound
    clips' edges, the playhead; Shift drags it free.
  - **No sliding of a cut's sound** against its picture (user,
    2026-11-03): the sealed, magnetic edit turned out handy, the cut never
    breaks; another clip's sound comes in by cutting it on the timeline.
  - Found on the way and fixed: a cut whose only sound change was a fade
    exported without it (the mix path wasn't taken).
  - Not done: Premiere's Alt-drag on the picture's edge; a rolling split
    (one drag lengthening one sound and shortening the next); automatic
    short fades at the moved edges (the fade knobs do it by hand).
- **What grouped and ungrouped look like** (user, 2026-11-02, checking
  timeline pack 1): grouped, the selection frame goes round the picture
  only, its sound under it without one: it goes with the picture. Once
  ungrouped (or split, J/L), the sound gets a frame of its own when
  selected, as any clip has: a sign that it now moves, trims and goes
  on its own (taken away, slid, or kept under another picture cut into
  its place). Pack 1's `selection_frame` and trim handles are what it
  draws; the clip sound lane's block then takes presses for its own
  edges.
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

**Built 2026-11-03** (`5262bc4`, `dabd3b8`, `264a376`, `8188d53`), waiting for
the user's check: `Cut::marks` ({t, note}, clip time) and `CutOp::Mark /
MoveMark / Unmark`; M marks at the playhead (mute moved to Shift+M,
FCP / Premiere / Resolve's key); flags on the time ruler (gap 50a)
rather than the seek bar, with the note beside them; a click goes to
one, a drag moves it within its cut; a double- or right-click opens a
window to name or delete it; ↑ / ↓ stop at them. Left: footage-agent's
flags exported into them; snapping to them (pack 3).

After the user's check (2026-11-03): the flags moved into a «Markers»
row of their own over the ruler (the playhead's head covered them on
it), folding to a strip, shown only with markers; the ruler keeps their
poles. A click on a flag selects the marker (`Selection::Mark`), so Del
takes it rather than the cut under the playhead; flags and notes 15 %
bigger, outlined (`768576a`). Found with it: shortcuts with letters did
nothing in the Ukrainian layout (Ctrl+Z among them; fixed by the
physical key, `keys::latin`), and ↑ / ↓ were lost in editing while the
angles strip was on in Settings (`c05423d`). ↑ / ↓ then stood still at a stop off
the frame grid (a sound clip's end, a marker): a stop counts only more
than half a frame away now, and the playhead says what it stopped at
(«← marker», «← cut»), with a test walking every stop both ways; then,
at the user's ask, only markers and cuts' joins stop it, not sound or
overlay edges. Confirmed by the user 2026-11-03, a cut's markers going with it
when it moves included.

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

**Built 2026-11-02 with timeline pack 1** (`b5c95f8`), confirmed by the
user the same day: 3 px in the accent colour over a dark 5 px line, on cuts,
sound clips and overlay items (`selection_frame`).

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


**Pack 1 built 2026-11-02** (`33fe0d0`, `b5c95f8`, `2094684`, and
`0eb941c` after the user's check: fade knobs kept 8 px inside so two at
a join don't stack), confirmed by the user the same day (the yellow /
red handles: «геніально»): edges a 24 px target (`EDGE` 12 px inside, a
cut's as far outside; one `edge_of` for cuts, sound clips and items; a
narrow clip split in thirds so its middle moves it); trim handles and
trimming only on the selected clip, yellow where there's footage
beyond, red at the file's end; the thick selection frame (gap 7); a
readout by the pointer while trimming, moving, crossfading or fading.
Defaults: a press on another clip's edge selects and moves it; pictures
and text show yellow at their out edge (they have no end).

**Pack 2 built 2026-11-02** (`994f949`, `afb5ac2`, `906cab3`, and the
toast's commit), confirmed by the user the same day: the crossfade
readout says «← onto the cut before» while it's 0 (the user dragged it
inwards, as a sound's fade in); Alt+[ / Alt+] trim the selected (else
playing) cut's start or end to the playhead, Alt+← / Alt+→ move it one
place; Esc cancels a drag on the timeline (`Edit::cancel`,
`drag_epoch`; not the effects sliders, iced's own); «… deleted · Undo»
in the top bar for 6 s after a delete. From the check: gap 55.

**Pack 3 done 2026-11-03** (`62540fe`), confirmed by the user 2026-11-04:
snapping across the timeline, one rule for every drag
(`Timeline::snap`): within 12 px (raised from 8 at the user's word) of
the edit's ends, the playhead, the pictures' joins, cuts' sounds split
from their pictures, markers, sound clips, overlays and text; never the
dragged thing's own edges or what moves with it. What snaps: the
playhead dragged, a cut's out edge (its in edge stays put on screen as
it trims, so it doesn't), a cut's sound edge, sound clips moved and
trimmed, overlays and text moved and trimmed, markers moved. A yellow
line runs through every lane, the ruler and the markers' row where a
drag snapped (`Message::Snapped`), gone at the drag's end or Esc. N
toggles snapping (kept in `settings.json`), shown by the magnet button
beside the zoom; Shift held lets one drag go free. Not yet: keyframe
marks, and beats (gap 57).

### 10. Two modes: Edit and Framing — built 2026-10-24 (`562dad6`); the base that gap 53 grows (user, 2026-11-02)

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
- **User's test, 2026-10-29:** the dialog ran on all 39 Kos cuts, evened
  out: "39 cuts done. Levels from −3.4 to +5.7 dB. 2 without sound". The
  lane tags show ("−2.8 · even"). The waveforms of scooter clips fill the
  lane: wind noise, checked in the cache (median peak 0.6–0.7, against
  0.01–0.05 for the talking clips). Then the right-click menu: «Lower for
  voice-over» and «Back to normal level» work both ways (on a scooter
  clip). Mute and «Show volume line» with points work too (2026-10-29).
  Card done. Left: the user's ear on the leveler (samples A–D).
- **Step 1 built** (`18671af`): `Cut.gain/mute/loudness`, `loudness.rs`,
  `rosmo360 normalize`, the export limiter (0.87 = −1.2 dBFS, `level=0`,
  `latency=1`), the player's limiter. Measured on Kos 22.08 cuts 1–6:
  −32.5 → −16.1 LUFS, true peak −1.0 dBTP, sync 0.0 ms.
- **The user asked the same as step 2 (2026-10-28):** "can it normalize by
  the dB of each stretch, so where it's loud enough it isn't raised and
  where it's quiet it is?" Measured on Kos, LRA = how far quiet and loud
  are apart:

  | Filter | DJI_0001 voice | DJI_0005 |
  |---|---|---|
  | none | 16.0 LU | 10.9 LU |
  | gentle `dynaudnorm=f=250:g=15:p=0.9:m=8` | 12.7 | 6.8 |
  | strong `speechnorm=e=12.5:r=0.0001:l=1` | 11.8 | 3.3 |

  **Correction (2026-10-28, after the user heard no difference):**
  - Measured moment to moment, the gentle B lifted quiet words only 2.5 dB.
  - Worse, dynaudnorm's default boundary ramp made **every cut start 13–15
    dB quiet** for 1–2 s in the export.
  - Fixed in `e78442b`: `b=1`, and the default is now the medium
    `dynaudnorm=f=150:g=7:p=0.9:m=12:b=1`. Quiet words go up +5.5 dB and
    the cut starts match the plain export.

  The samples were regenerated: A plain, B gentle, **C medium (the
  default)**, D speechnorm. Listen around 28–34 s of the voice.
  Old note:
  Listening samples, each at −16 LUFS, so only the character differs:
  `~/omnix-samples/out/levelers/{voice-0001,clip-0005}-{A-plain,B-gentle,
  C-strong}.flac`. Default until the user picks: B, gentle. The strong one
  may pump and lift the noise between words.
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
- **25f. A Transitions lane** (user, 2026-11-01): "transitions could also
  be our own collapsible field somewhere above the effects. Why: on the
  transition line you draw a rectangle (over the join of one video and the
  next) and the program fills it with a transition we like: either a video
  manipulation (out-in) or a video insert (white-video-noise), whose sound
  goes straight onto the SFX lane."
  - A thin lane of its own, folding like gap 39's groups, above Effects.
    It replaces `TRANSITIONS.md`'s «transition mode» (T toggling handles
    on the cut row): the zones live on this lane, so the cut row stays as
    it is.
  - Drag a rectangle across a join → it's filled at once with the
    default template (the last used, or a chosen favourite); the rectangle's
    width is the transition's length, snapped to 0.25 s. A click on it
    opens the presets panel to change it.
  - Kinds as `TRANSITIONS.md` has them, chosen by what the template is:
    an overlap or edge transition (out-in, dissolve, wipe…) or a stinger
    (an inserted clip: white noise, film burn).
  - A stinger's sound is placed on the SFX lane at the same time and
    length, linked to it: moving or deleting the stinger moves or deletes
    its sound; on SFX it can still be levelled and faded like any clip.
  - The three quick ways (T at the playhead, between two selected cuts,
    on every join) fill the same lane.

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
- **Grouped effects per track** (the FL idea; designed in gap 39): a cut's effects row could
  show lanes per automated value (exposure, speed, volume) that fold open,
  instead of one row of marks. A UI question, together with gap 9's action
  bar and the effects panel.

### 31b–31e. Speed: the pitch choice, a speed editor window, keyframes by right-click — large (user, 2026-11-01)

User: "changing the speed of a video, with and without keeping the
sound's timbre. I don't much like Kdenlive's variable speed: I'd like it
in a separate window, with a speed curve and a seconds–milliseconds ruler
on top (the scale changes with the speed of that part of the video). And
right-click on the timeline in the one-clip editor should insert
keyframes too. We keep both: Kdenlive's way and the kind of speed editing
I want."

- **31b. Keep the timbre, or not.** A per-cut switch, «Keep the voice's
  pitch», on by default:
  - on: the sound is stretched in time, the pitch stays (`atempo`, or
    `rubberband` for ramps);
  - off: like tape or a record, slower is lower (`asetrate` + resample).
    Good for effect: a slowed-down fall, a sped-up silly walk.
  - Optional field `Cut.keep_pitch` (absent = on), so old projects load.
  - This machine's ffmpeg 6.1.1 has `rubberband` with runtime commands
    (`C` in `ffmpeg -filters`), so its `tempo` can follow a ramp through
    `sendcmd`. Not every build has it: check at start-up and fall back to
    constant-tempo pieces with `atempo`.
- **31c. The speed editor, a window of its own.** Opened from a cut (its
  menu, or a double-click on the speed lane).
  - The cut's picture at the top, as in the one-clip editor.
  - Under it, a **speed curve**: speed up the side (0.1× to 4×, a log
    scale so 0.5× and 2× sit at the same distance from 1×), time across.
    Points are dragged; the segment between two points eases (the same
    easings as the view keyframes, gap 17).
  - Above the curve, a **ruler in the footage's own seconds and
    milliseconds**, its ticks spaced by how long each stretch plays: at
    0.5× a second of footage takes twice the width, at 2× half. So you see
    at a glance where the time is stretched and where it is squeezed.
  - A second, plain ruler of the edit's time below it, so both clocks show.
  - The cut's resulting length is shown live; the cuts after it move as it
    changes, as the magnetic timeline does now.
  - Presets: «slow in the middle» (ramp down and up), «speed up to the
    cut», «freeze» (0× for N seconds: a held frame, the sound silent).
- **31d. Keyframes by right-click on the one-clip timeline.** In the
  editor of one video (and in the speed window), a right-click on the
  timeline opens a menu: «Add a speed point here», «Add a view
  keyframe», «Add an effect keyframe», «Add a volume point». Today these
  are added with Ctrl+click and buttons; the menu makes them findable.
- **31e. Kdenlive's way stays too.** Speed keyframes right on the cut in
  the main timeline (its speed lane folding open under the cut, like
  volume points), for a quick change without the window. Both edit the
  same data: one list of speed points per cut, in the footage's time.
- **Data.** `Cut.speed_keys: Vec<SpeedKey>` (footage time, speed, ease),
  optional; a constant speed is one point. The edit's time is the
  integral of 1/speed over the footage (31's mapping, every place that
  converts edit ↔ clip time), computed once per change and cached as a
  table, so the player's clock and both exports read the same numbers.
- **Video.** Frames are picked by time already. Slower than the clip's
  frame rate allows (30 fps at 0.3×): repeat frames, blend them, or
  `minterpolate` (motion-interpolated, slow to render; export only). The
  choice per cut, default repeat.
- **Order.** 31 (constant speed) and 31b first; then the data of ramps
  and 31e on the main timeline; then the window 31c; 31d whenever the
  one-clip editor gets its menu.

### 32. A blank stretch on the main lane — done 2026-11-03 (`1c2a457`), confirmed by the user

Asked by the user 2026-10-23, again as a «spacer» 2026-11-03: a black
stretch on the main lane, room for an overlay, a title or a sound with a
custom stinger over it.

How it works:
- The top bar's **+Blank** (after +Clip) writes `Blank.png` (320×180,
  black) once into the project's `08_graphics/` (before the first save,
  the cache's `blank/`) and adds it as a cut at the playhead, by the
  bin's route (`CutOp::Add`, `Place::At`). At a join it goes between the
  cuts; mid-cut it splits the cut. It is a picture: 5 s, trimmed at will,
  named «Blank» on the timeline from the file's stem. One file serves
  every blank.
- `rosmo360 blank DIR` makes the same file and prints its path, so a
  script or an agent adds one with a cut `add` op.
- **The magnetic edit stays** (user, 2026-10-23: "handy"): gaps happen
  only as blank cuts.

Decided by default: no colour choice and no length dialog (drag the edge;
the user can ask for either). A blank made before the first save keeps
its path in the cache.

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

### 33b. Show a cut's volume line from the menu — built 2026-10-29 (user)

User: "show the sound line from an option in the context menu (it'd start
at 0 dB), so I can pull the level up with the curve here and there".

Built (see the commit): «Show volume line» puts points at the cut's in and
out at 0 dB. A click on the drawn line adds a point and drags it, without
Ctrl. Later, the same for the music and SFX clips' menus.

### 33c. Fade handles on a cut's own sound — small-medium (user, 2026-11-01)

User, checking the fade handles: "the fade handles work; missing: fade
handles on the video's sound." The same white corner handles as on sound
clips, on the Clip sound lane: the cut's sound fades in at its start and
out at its end. Optional `Cut.sound_fade_in/out` (seconds, absent = 0);
the player's clip volume and the export's mix both apply it (CPU = export
rule), and a crossfade's own sound blend stays as it is.

**Built 2026-11-01** (`a8d9e6f`), confirmed by the user 2026-11-01. Straight fades, as
the sound clips'; handles shown only where the cut has sound; a split
keeps each fade at its half's outer end; the waveform shows the fade.
Export checked: 2 s fades on the talk cut measured −50 dB at its start,
full 2 s in, down again at its end.

### 33e. A sound clip's own volume line, apart from the track's — medium (user, 2026-11-01)

User (screenshot of the Voice lane: the yellow volume line starts at the
lane's left edge, over empty space, not at the take's start): "so it
isn't like this (the curve goes from the start of the lane, not from the
start of the file), I'd add a context menu with a few items: «continue the
volume from the previous file», and then a sound lane appears on the new
file; or «reset the volume and sound to the original». I don't know if
that's right, you tell me."

Why it looks so: the line is the **track's** (`Music.keyframes`, edit
time, the whole lane), not the clip's. One point on Voice at 12.8 s holds
its +3.1 dB from the lane's very start. That is how a DAW's track
automation works (FL, Ardour), and it suits a mix move over several
clips (a duck under speech). It is wrong for "this take a little
louder": the level should belong to the take and move with it.

**Recommended: both, as cuts already have** (`Cut.volume` lives in the
clip, the track's line over the whole edit):
- **The clip's own line** (default). A sound clip gets optional
  `MusicClip.volume: Vec<VolumeKey>` in the file's time, so it moves and
  trims with the clip and is drawn only inside it. «Show volume line» in
  the clip's right-click menu puts points at its in and out at 0 dB (as
  33b does for cuts); a click on the line adds points.
- **The track's line** stays, for moves across clips. Drawn dimmer, and
  only when shown from the lane's name menu («Show track volume»), so it
  no longer looks like it belongs to a clip. Existing track points keep
  working, so old projects sound the same.
- **The menu on a sound clip** (the user's items, kept):
  - «Show volume line»;
  - «Continue from the previous clip»: the clip's line starts at the level
    the clip before it on the lane ends at, so a split or a second take
    carries on without a jump;
  - «Back to the original»: the clip's points, gain and fades cleared;
  - also «Lower −12 dB», «Mute», as on Clip sound (gap 33), and «Info».
- Player and export apply clip line × track line × track level (CPU =
  export rule, tested on the mix graph strings).

**Built 2026-11-01 with 33f, confirmed by the user the same day** (`4e0411b`, `37da672`, `d3902f0`, `23fee09`,
`9bc8478`, `8046051`). As designed, with
these defaults: the clip's line is cyan like a cut's (both «own» lines),
the track's line stays shown when it has points but dimmer (no separate
«Show track volume» switch: hiding it would hide existing points); the
lane's empty stretch has the track's menu with «Remove the track's
volume line». Ctrl+click on a clip adds a clip point, off the clips a
track point. «Continue the level of the one before» counts the one
before's own level and points, not the track line. One set of
operations serves cuts and sound clips (`project::Own`, `OwnSound`).

### 33f. Normalize and even out sound clips, the voice-over above all — medium (user, 2026-11-01)

User: "I'd add a context menu and normalizing for the voice-over (one
take may be further from the mic, another closer, so they need to be made
even). And as before, normalize and level up all the sound clips on the
track: it would fit here."

What it is: what Clip sound got in gaps 11, 12 and 33, for the clips on
Music, SFX and Voice.
- **Data:** `MusicClip.gain` (dB), `.even` (through `LEVELER`), `.mute`,
  `.loudness` (what normalizing measured), all optional, as on a cut.
  They join 33e's own volume line on the clip.
- **The clip's menu** (33e's, right-click a sound clip): «Normalize»,
  «Even out», «Lower −12 dB», «Back to normal», «Mute», plus 33e's
  «Show volume line», «Continue from the previous clip», «Back to the
  original».
- **The track's name menu** (right-click the lane's name tag): «Normalize
  every clip» with «Even out first», the same dialog with progress as for
  cuts (`sound_ui.rs`, one undo step, one clip at a time).
- **Target.** Voice and SFX to −16 LUFS like the cuts, so a take recorded
  further from the mic comes up to the closer one. Music too when asked,
  but the music's level stays the track slider's job (normalized music
  under speech is then lowered by auto-duck or the slider).
- **Player and export:** the clip's gain and evening in the music player's
  stream (decoded through the leveler when evened) and in the export's
  per-clip chain, before its fades and the track's level; the waveform
  drawn as heard, as 33d does for cuts.
- Built together with 33e: one menu, one per-clip level model.
- **Built 2026-11-01** (see 33e's commits), confirmed by the user.
  Right-click a sound clip for its menu; a sound lane's empty stretch for
  «Normalize every clip on …». Same −16 LUFS target and dialog as cuts.
  Export checked: a take at −20 dB, evened, peaked at −20 dB.

### 33g. Removing points and lines, and the menu where the pointer is — small (user, 2026-11-01)

User, checking 33e/33f (confirmed: the clip's cyan line goes with it, the
track's yellow line unchanged; normalizing «works perfectly», also on a
take recorded without manual gain):
- Ctrl+click on an existing point on any lane removes it.
- Right-click on a point: «Delete point», a menu item of its own, so a
  misclick doesn't remove it.
- Right-click on a line: «Delete the whole line», likewise.
- Bug: a right-click on a lane opened the menu near the top of the Music
  lane, not at the pointer. Cause: the menu was put a fixed 240 px above
  the pointer whatever its height. Fix: its bottom at the pointer, by its
  own height (items × row height), kept inside the window.

Built 2026-11-01 (`4f50a2e`), confirmed by the user the same day. A right-click near a cut's or
clip's drawn line now opens the line's menu; elsewhere on it, its sound
menu as before.

### 33h. Bending a line between two points into a curve — idea (user, 2026-11-01)

User: "dragging the line between points (click and drag the line up or
down bends it according to where you drag) makes a smooth curve."

As FL Studio's automation and Ableton's curved segments: a segment's
shape, not only its ends.
- **Gesture.** Today a click on a line adds a point there. Proposed: press
  on a segment and drag up or down → it bends (the further, the stronger);
  a click without moving still adds a point. Double-click on a bent
  segment makes it straight again.
- **Data.** An optional `curve` on a point (−1..1, absent = straight),
  shaping the segment after it: in dB, `a + (b − a)·shape(x)`, with
  `shape(x) = x^(2^(curve·k))` or an equivalent that bends toward one end
  as dragged. Old projects load straight.
- **Same in the export** (CPU = export rule): `level_filter` writes the
  curve into its per-frame expression (`pow` in ffmpeg's expression
  language), tested on the string as now.
- **Everywhere lines are:** a cut's and a sound clip's own lines, the
  track lines, and later every automation lane (gap 39's generic `Curve`
  gets it once, so effects and speed curves bend the same way).
- **Approved by the user 2026-11-01** ("like in FL, nice, approved"):
  built with the automation lanes, volume lines first.

### 33i. No track volume line: each clip's own line only — medium (user, 2026-11-01)

User, checking 33e's points: "clicks inside a clip edit only the cyan
line; to edit the yellow (track) line you click outside the clip. I
think we should remove the track line altogether: it works and overlaps
the clip's. Clips have the functions anyway." Also: "K works only for
the yellow track line" (with a clip selected, K added to the track's
line, which was the clip's «main» line), and the Audio panel's «Lower
under speech» wasn't found (it is under the speaker button in the
transport row; to make it easier to find).

Decided:
- The track volume line goes: no drawing, no clicks, no menu item.
- **Old projects keep their sound:** on load a track's points are moved
  into the line of each clip on it (the clip's own level plus the track
  line at each point's time, at its start and end and at every track
  point inside it), and the track's points are dropped. The field is
  still read, never written.
- **Auto-duck** writes into the Music clips' own lines.
- **K** with a sound clip selected adds a point on its own line.
- The track's level slider (Audio panel) stays: the whole track up or
  down.
- `project::Line` (Own or Track) folds back into `Own`.

**Built 2026-11-01** (`c34ae65`, `abc21b9`), confirmed by the user with the sound group. On
the test project the music's six track points and the Voice point moved
into the clips' lines; auto-duck writes there. «Lower the music under
speech» is now also in a song's right-click menu.

### 33d. Waveforms drawn as they're heard — small (user, 2026-11-01)

User, after normalizing the scooter cut and lowering it for voice-over:
"I don't know whether the leveler changed anything." The lane draws the
file's raw waveform, so −14.4 dB and «even» look the same as before.
Draw it scaled by the cut's gain and volume points (and its mute), and,
when evened, from the evened sound (the waveform read through
`LEVELER`, cached beside the raw one). Then the change is visible.

**Built 2026-11-01** (`ded823b`), confirmed by the user 2026-11-01. Defaults
taken: the file's own waveform stays faint behind the heard one, so a
change shows against what was there; bars a gain pushes past full scale
are orange (the export's limiter holds them); linear height as before
(not dB), so −14 dB shows as a fifth of the height. The evened waveform
is the whole file read through `LEVELER`, cached as `wave-even`; the mix
evens each cut's stretch, so the very start of a cut may differ a little.
Sound clips (Music, SFX, Voice) are drawn with their track's level,
points and fades too.

Why the leveler didn't seem to change the scooter: its sound is wind,
loud all the time. The leveler lifts quiet stretches toward loud ones;
with no quiet stretch, there is nothing to lift. It helps on talk cuts
with near and far voices. Wind needs noise reduction instead (a later
«Clean up sound» switch, gap 36's «later» list).

### 36. Record a voice-over inside Omnix — medium-large (user, 2026-10-29)

User: "for the voice-over there should be a way to record right here in
the edit: the microphone opens a context menu with 'add from disk' or
'record here in the editor'. Look at good examples of such windows. Can
be a side panel of its own, and the sound written straight into the
Voice lane on the timeline. It must show what recording device is
connected (Bluetooth mic, audio interface, etc.)."

**What good editors do** (research, 2026-10-29: Final Cut's Record
Voiceover, Resolve's Edit-page voiceover, Premiere's track mic button,
iMovie, CapCut, Audacity, Kdenlive, Shotcut, OBS):
- a 3-2-1 countdown;
- the edit plays while you record, with "use headphones" or "mute the
  project";
- a big input meter with a clip warning, alive *before* Record;
- the device defaults to the system's and is remembered;
- takes are named and numbered automatically;
- the playhead goes back to the start after Stop;
- noise cleanup is one switch afterwards;
- Audacity alone is open about latency: it shifts the take after
  recording.

Shotcut is the warning: no device picker, no meter, and users get lost.

**Design for Omnix:**
- **Entry.** The microphone button opens the Record panel at the right.
  Its first line is «Add from disk…», so the button gives both, as asked.
- **The panel, top to bottom:**
  1. Input: friendly name with its kind (Bluetooth / USB / built-in /
     webcam), starting on the default and then remembered.
  2. A warning when needed. Bluetooth: phone quality, 16 kHz, and the
     headset's playback drops too while recording. A webcam: low quality.
  3. A live level meter with a peak mark and a CLIP light, running while
     the panel is open.
  4. Channel (1 / 2 / both) for a two-channel interface.
  5. Countdown 3 s (on); Play the edit while recording, use headphones
     (on).
  6. A big Record / Stop button, with the elapsed time.
- **Flow.**
  - Record starts ffmpeg at once, as a warm-up: opening the device, and
    Bluetooth's profile switch, take 0.3–2 s.
  - The countdown runs, then the edit plays from the playhead T.
  - On Stop, ffmpeg gets `q` (a clean WAV).
  - The take lands on the Voice lane at T, its in point past the warm-up
    and the countdown and corrected for latency, as one undo step. The
    playhead goes back to T.
- **File.** `02_audio/voiceover/VO_<date-time>_take<NN>.wav`, 48 kHz,
  24-bit, mono, never overwritten. It is kept whole and trimmed only on
  the timeline.
- **This machine** (PipeWire 1.0.5, ffmpeg 6.1.1):
  - Inputs: the Scarlett Solo (USB, 2 channels, the mic on channel 1)
    and the webcam's mic (16 kHz mono). The built-in card's profile is
    `off`, so its mic isn't listed.
  - `pactl -f json list sources`, keeping `media.class == Audio/Source`;
    `device.bus` is bluetooth / usb / pci and `device.form_factor` says
    webcam or headset; hot-plug arrives on `pactl -f json subscribe`.
  - Recording: `ffmpeg -f pulse -i <source>`, one channel with
    `pan=mono|c0=cN`, `asplit` into the WAV (`pcm_s24le`, `-rf64 auto`)
    and into `astats` metadata, printed in ffmpeg's log, for the meter.
    Writing `q` to stdin stops it cleanly (tested).
- **Latency.** Speaking along to playback, the voice is late by the output
  plus input latency: 20–60 ms wired, 150–300 ms on Bluetooth. The in
  point adds the device's output latency (cpal knows it) and an input
  estimate. A «Voice sync (ms)» setting comes later if it's audibly off.
- **Bluetooth.**
  - A2DP has no mic.
  - Using the headset's mic switches it to HFP/HSP: mono, 8 or 16 kHz,
    and playback drops to that too. WirePlumber switches on its own when
    the mic opens; the warm-up covers it.
  - A USB mic for input and the Bluetooth headset for listening is the
    good pair.
- **Built 2026-10-29** (`35735d6` recorder, `084c617` panel), waiting
  for the user's check. `src/capture.rs` lists the inputs and records;
  `src/player/record_ui.rs` is the panel. Defaults taken alone, to change
  if wanted:
  - the latency is a fixed estimate, 0.04 s wired and 0.25 s Bluetooth,
    not measured from cpal; plus 0.05 s for the meter's reading window
    (the first sound is dated by its first level line);
  - where the edit was at the playhead is read off the playback's own
    clock in its first second, so a slow start doesn't shift the take;
  - countdown and play-along on; input, channel and both switches
    remembered in the settings;
  - Record is off until the project is saved (takes go in its folder);
  - closing the panel mid-take stops it and keeps the take;
  - a take shorter than 0.1 s after the countdown is dropped with a
    message (the file stays on disk);
  - the meter's floor is −60 dBFS; CLIP lights at −0.1 dBFS and stays on
    until clicked;
  - Linux only (PipeWire/Pulse via `pactl`); other systems say so.
- **Found while building:** ffmpeg's `ametadata=file=/dev/stderr` sits in
  a 32 KB buffer, so the meter got nothing for minutes. The levels come
  through ffmpeg's log (`-loglevel info`) instead, live.
- **Checked by the user 2026-11-01** with the webcam's mic: works. They
  asked for a manual gain (36b): the webcam peaks at −12 dB at most.
- **36b. Input gain.** A slider in the panel setting the input's own
  volume in PipeWire (`pactl set-source-volume`, 0–150 %), the same the
  system's sound settings change, so the meter shows it at once and the
  take is recorded with it; a boost above 100 % is digital and raises the
  noise too.
  **Built 2026-11-01** (`93365ce`), confirmed by the user 2026-11-01. Range 0–200 %
  (not 150: the webcam was already at 100 % when it peaked at −12 dB, and
  PipeWire's 200 % is ×8, +18 dB). Read again when the input changes; the
  slider stays usable while recording, like a knob on an interface.
- **Later:**
  - takes stacked as alternates, and a takes list;
  - «Clean up voice» (footage-agent's cleaner, or `afftdn`);
  - enabling a disabled built-in card's profile.

### 37. Object tracking — large (user, 2026-11-01); part of gap 53 (53b) since 2026-11-02

User: "object tracking" (recalled with the speed ideas; no detail yet).

What exists: `rosmo360 track` and the colour-track picker follow one
**colour** in a 360 clip and key the view to it (`src/track.rs`). It
works on a jacket or a tent; it loses a subject in a crowd of similar
colours, and does nothing for a flat video.

What tracking would add, in the order it seems most useful:
1. **Framing follows an object on any video.** Draw a box on the subject;
   the box is followed frame by frame (a template / optical-flow tracker:
   KLT or a correlation tracker written in Rust, no OpenCV dependency) and
   the view keys to it. On a flat video that is a moving crop: 16:9 → 9:16
   for a phone, the subject kept in the frame.
2. **Things stuck to an object:** text, a picture, an arrow, or a blur
   (a face, a number plate) following it. The overlay's position keys
   come from the track.
3. **Correcting a track by hand:** the track shown as a path; a wrong
   stretch re-done from a point, or keyed by hand.

Open questions for the user: which of these is meant first; is it for
360 clips, flat ones, or both. Default if not answered: 1 on flat video
first, since the colour tracker already covers the simple 360 case.
Heavy computing goes off the UI thread with progress, as the colour
track does; a face or person detector (footage-agent's gap 30 models)
could seed the box later.

### 38. A slider dragged during playback makes a key per frame — bug, small (user, 2026-11-01)

Seen in the test project's saved file: dragging the vignette slider while
the edit played left 9 effect keys on one cut and 15 on another, 0.03–
0.07 s apart. The user confirmed they dragged during playback.

Cause: `effects.rs` `change_effects` (and `change_place` for overlays)
writes into the key at the playhead, or makes one there, on every slider
message. The playhead moves while playing, so each step lands at a new
time.

Fix (default): one drag is one key. The time is taken when the drag
starts (the playhead then) and kept until the slider is let go
(`EffectMsg::Done`); every step of that drag changes that one key. Same
for the overlay place sliders and the on-picture drag. A test: a drag
of many steps with the time moving between them leaves one key.

**Built 2026-11-01** (`b070d97`): `effects::Pin` keeps the drag's target
and time from its first step; one-shot changes (reset, white balance, key)
take the playhead as before. Two tests. Confirmed by the user 2026-11-01.

Recording a control's moves during playback as a curve (FL Studio's
«record automation») goes to the automation lanes, gap 39 (user,
2026-11-01): it writes into one parameter's curve there, thinned to the
few points that keep its shape.

### 39. Automation lanes: Effects and View as folding groups of curves — large (user, 2026-11-01)

User (pointing at the two thin rows «Effects» and «View» under the clips,
which show keyframes as dots): "I once suggested these become group
spaces, where under them, as in FL Studio, you could add a view of the
automation curve for each effect; I think that's very handy. And make
both of them collapsible. So what we have in the effects now would just
render as separate 'fields' with curves, instead of just placing points.
View doesn't work now, but the function, contract and factory can be
thought out for it too." (First raised 2026-10-23, gap 31's «grouped
effects per track».)

**What it looks like.**
- «Effects» and «View» stay as one thin row each, with a ▸ to fold open.
  Folded, as today: the dots of all keys.
- Open, a lane per parameter that is automated (exposure, contrast,
  vignette… for Effects; yaw, pitch, roll, FOV for View), each with its
  name, its curve over the cut, and the value under the mouse. A «+»
  at the group's head adds a lane for a parameter not yet automated.
- In a lane: a click on the line adds a point (as the volume line,
  33b); drag a point in time and value; right-click for delete and the
  easing (gap 17); double-click resets to the default.
- Lanes are as tall as they need; the group remembers open or folded per
  project.

**The data question (to decide before building).** Today one effect key
holds *every* parameter at its time (`grade::EffectKey` = time + a whole
`Grade`; a field left out of the JSON means 0). So a vignette point and an
exposure point can't sit at different times independently; drawing lanes
over this model would move all parameters when one point moves. Two ways:
- **A (proposed): a point list per parameter**, as FL does: an optional
  `automation: { "vignette": [{t, v, ease}], … }` per cut (and per
  overlay). Old `effects` keys are read into it on load (each key gives a
  point to each parameter it changed); the old field is still read, so
  old projects open. Independent curves, and the gap 38 «record
  automation» mode writes into one lane alone.
- B: lanes as views of the shared keys. Small, but coupled: moving one
  point moves them all. Not what the user described.

**The contract («factory»).** One generic lane, so a new automatable value
is one description, not a new widget:
- `Param`: id, name, range, unit, default, scale (linear, dB, log for
  speed), how it is applied (into a `Grade`, a `View`, a gain).
- `Curve`: points `{t, v, ease}` with `value_at(t)`, add, move between
  neighbours, remove: the volume-point functions of the edit API
  (`ARCHITECTURE.md` step 5) made generic.
- The timeline's lane drawing and mouse handling take a `Param` + `Curve`
  (`ARCHITECTURE.md` step 8, «timeline lanes behind one lane type»).
- Users of it: Effects (11 parameters), View (yaw, pitch, roll, FOV),
  a cut's volume and the tracks' volume (already curves), speed (gap
  31), overlay motion and opacity, and later a stinger's or transition's
  parameters.

**View today.** The user says View «doesn't work now»: its row shows the
view keys of 360 framing, but turning the view by keys (automation of
framing) is what's missing on flat video and in Edit mode (noted with the
Framing check, 2026-11-01; tracking, gap 37, comes with it). The
contract covers it so View lanes arrive with that work.

**Order.** The generic `Curve` and lane first (moving volume onto it, no
change seen), then Effects lanes (data A with conversion), then View
lanes with gap 37, then speed (31) as one more `Param`.

### 40. Other platforms: Mac, Windows, an Android tablet or phone — question (user, 2026-11-01)

User: "could this program move to Android (a tablet), or to Mac/Windows if
needed? Or an Android phone (we have the functions, only a separate
minimal phone UI to write)."

Where it stands (answered 2026-11-01, not planned):
- **Mac and Windows: realistic.** Upstream builds both (`CLAUDE.md`
  item 3: an arm64 `.app` tested on the user's MacBook, a Windows zip
  with its own ffmpeg). Omnix's own Linux-only parts each have a stub
  that says so: the voice-over inputs and gain (`capture.rs`, PipeWire's
  `pactl`; Windows/macOS would list ffmpeg's dshow / avfoundation
  devices). Everything else is portable by the project's rule.
- **Android tablet: possible, large.** The library crate (gap 35's
  refactor) is the part that carries over: the project format, the edit
  API, the export's maths, the GPU stitch (wgpu runs on Android's
  Vulkan). What doesn't: iced has no maintained Android target, so the
  UI would be a new shell (Kotlin/Compose over the Rust core, or a
  touch-first Rust UI); ffmpeg as a separate program is awkward there
  (an app ships it as a library, which brings back the licence choice
  upstream avoided); file access goes through Android's storage rules.
- **Android phone, minimal UI:** the same core and the same costs, plus
  a small screen: a «trim, order, music, export» editor rather than the
  full one. Cheapest of the three on UI, same price on the plumbing.
- What keeps the door open now: the edit API (`ARCHITECTURE.md` step 5)
  and no new Linux-only code without a stub.

### 41. Core, functions and API: what to unify and separate — analysis (user, 2026-11-01)

User: "run a check and put into a separate file the possibilities of
unifying and separating out the core / functionality / API, which will
make further building simpler for us and divide the architecture
nicely." The second pass after gap 35's refactor, now that sound clips,
recording and the shared sound model (`project::Own`) are in. The result
goes to `CORE-API.md`; the steps it proposes become commits here. Analysis
done 2026-11-01 (`95a6be7`): ten steps, tests first; it also found the
contact sheet leaving a sound clip's own level out.

### 42. Unit and integration tests — analysis and adding them (user, 2026-11-01)

User: "in parallel, an analysis for unit and integration testing; where
we don't have such tests, add them, so refactoring and improving happen
more calmly." An inventory and a plan in `TESTING.md`, then the tests
in small commits, riskiest first (time mapping, project load/save and
old JSON, the export's sound graph, an end-to-end CLI export on tiny
generated clips).

**Done 2026-11-01:** the whole plan in `TESTING.md` (84 → 117 tests,
three of them end to end on generated clips). Three bugs found and
fixed: paths of a project saved elsewhere; sound clips cut off by the
edit's end sounding different in the export; levels near 0 dB that
couldn't be set on narrow lanes.

### 43. Editing-software best practices, colours, a switchable UI palette — research (user, 2026-11-01)

User: "analyse best practices for editing software, look at colour
schemes, theoretically add the ability to change the UI's colour
palette; the result can go to a separate file too." Research and a
proposal in `UI-COLOUR.md`: neutral surround around the picture,
semantic colour tokens instead of hard-coded colours, lane colours per
kind, a colour-blind-safe preset, palette presets in Settings with
per-lane overrides (iced's `Theme::custom`). Research done 2026-11-01 (`UI-COLOUR.md`):
also found two colours with two meanings (orange: overlays and «over full
scale»; cyan: the clip sound lane and a clip's own line) and Music's
colour following the theme.

### 44. A clip's overall level, by hand: a knob after the duck — small-medium (user, 2026-11-01)

User, after auto-duck drew the song's line: "some general knob per clip:
say I don't like that the music is still too strong, then after the
adjustment to the speech I could lower it overall (per clip)."

The model has it already: a clip's `gain` (dB) sits under its own line,
so the ducked shape stays and the whole clip moves; today only «Lower
−12 dB», «Back to normal» and Normalize set it. Proposed:
- every sound clip shows its level tag, as cuts do («−6.0 · even»);
- **drag the tag up or down** to change the clip's level, 0.5 dB steps
  (Shift: 0.1), the waveform following live; double-click: back to normal
  (what normalizing gave it, or 0);
- the same on a cut's tag on the Clip sound lane;
- «Level…» in the clip's menu to type a number.
One undo step per drag. Several clips selected (gap 8) later move
together.

**Built 2026-11-01** (`0f65503`), confirmed by the user with the sound group. Defaults: the tag
is always shown now (it was only when the level wasn't 0), 0.25 dB a
pixel in 0.5 dB steps (Shift 0.05 / 0.1), capped at +24 dB as normalizing
is; a volume point wins over the tag where both are. «Level…» to type a
number isn't needed (user). Confirmed by the user 2026-11-01: the knob,
the ducked curve kept, Shift.

### 45. A mix check: the sound's levels against a platform's, drawn and clickable — large (user, 2026-11-01)

User: "an analysis of the sound lanes, as a panel, to see what stands out
from the rest: say I forget to turn down a sound effect, or speech is too
quiet somewhere. Choose the overall values YouTube or other platforms
want for sounds, clips, music (apart and mixed) for a graphic panel.
Maybe interactive, in place of the video view: a schematic of the sound
lanes in their colours with hints where something should change by the
rules; a click on an element selects that clip on the timeline and
scrolls to it, to fix it by hand."

**What it measures** (ffmpeg's `ebur128`, off the UI thread, cached until
the edit changes): each clip's sound as heard (its level, line, fades,
evening; the export's own graph per lane, so it's what the export makes):
integrated and short-term loudness, true peak; the whole mix's integrated
loudness and true peak; per stretch of speech, the music and effects
under it relative to the speech.

**Targets** (presets, editable). Sources disagree; the most-cited:
- **YouTube, Spotify:** the whole mix about −14 LUFS integrated, true peak
  under −1 dBTP; louder is turned down, quieter isn't turned up.
- **Apple Music, Apple Podcasts:** −16 LUFS, −1 dBTP.
- **TikTok, Instagram:** unpublished; sources say −14 (some −10…−12).
- **Broadcast (EBU R128):** −23 LUFS.
- **Music under speech:** 15–20 dB below the voice is the common advice;
  WCAG 1.4.7 asks 20 dB for speech-led video. (Auto-duck's default 12 dB
  is under that: to reconsider with this.)
- **Speech:** steady across the edit (normalized to −16 LUFS today), no
  stretch more than a few LU under the rest.
- **Sound effects:** not louder than the speech around them by more than a
  few dB at peak.

**The view.** A switch beside «Framing» puts the mix check in the
viewer's place:
- the sound lanes as a schematic in their colours, each clip a block whose
  height is its loudness, bands for the target drawn behind;
- what breaks a rule marked on it, with the hint in words («SFX 9 dB over
  the speech», «speech 7 LU quieter than the rest», «music only 8 dB under
  the voice», «mix −19 LUFS: YouTube won't raise it»);
- a click on a block selects that clip on the timeline and scrolls it into
  view, to fix it by hand (its level knob, gap 44);
- the whole mix's loudness and true peak against the preset, at the top.

**Built in three steps** (started 2026-11-01): (1) the rules, pure and
tested, from measured numbers to findings; (2) the measuring: each lane
rendered alone through the export's own mix graph (so «as heard» is the
export's), its clips measured, plus `rosmo360 mixcheck`; (3) the view.
Built 2026-11-01 (`40ed95b` rules, `c145a2d` measuring and CLI, `f159edb`
view), confirmed by the user 2026-11-02. Defaults: the target is the Export panel's
loudness, else −14; a speech stretch's voice is the louder of the Voice
lane and the clips' sound; the music under it the song covering most of
it; measured when switched on and on «Measure again», not live. On the
test project: the film −14.1 LUFS, music 14.2 and 2.8 dB under the voice.
46f (the duck's depth) was set from it: 18 dB.
The user tried it (2026-11-02): «Lower the music under speech» and the
level knob at −3.5 dB cleared the red. Asked: a timeline in the check,
and its points on the timeline. Added (`6ece9f4`): each finding's stretch
(`Finding::until`), a ruler of seconds over the schematic with the
findings marked (clickable), and the same marks over the seek bar while
there are measurements.

### 46. More for the sound: what the review found — ideas (2026-11-01)

From going through what's there against what an editor of home video
reaches for (the user's question «what else may be missing for sound?»):
- **The export's overall loudness** (46a): a target for the whole mix
  (YouTube −14 LUFS) measured and applied at export, true peak −1 dBTP;
  today each cut is normalized to −16 and a limiter holds sample peaks at
  −1.2 dBFS, but nothing sets the whole.
- **A loudness meter** (46b): short-term and integrated LUFS beside the
  peak meter while playing, so «too quiet» shows in the units the targets
  use.
- **Low cut, wind** (46c): a high-pass switch per clip (wind on the scooter
  lives under ~150 Hz), and a «Clean up» (`afftdn` / `arnndn`) for steady
  noise; footage-agent's cleaner already makes the cleaned voices.
- **Solo and mute per lane** (46d): hear the Voice alone, or everything
  but the music, while mixing.
- **Built 2026-11-01, confirmed by the user:** 46d solo and mute
  (`2a085c0`): «S»/«M» by each sound lane's name, for listening only (not
  saved, not exported). 46c rumble cut and cleaning: `loudness::LOW_CUT`
  (highpass 120 Hz, 2 poles) and `CLEAN` (afftdn nr 12, nf −40, tn), one
  chain with evening, in playback, export and measuring; on the scooter
  −6.5 dB under 150 Hz, −3.4 LUFS overall; cleaning does nothing for wind.
  The waveform isn't drawn through them (only through the leveler).
- **46b built 2026-11-01** (`0a09f3c`), confirmed by the user: BS.1770 in
  `player/lufs.rs`, fed every played frame; short-term in the top bar,
  integrated in its tip; within 0.3 LU of ffmpeg on pink noise.
- **46a built 2026-11-01** (`31fb3bf`), confirmed by the user (Apple and social exports differ in loudness as they should): Export panel
  «Loudness» and `export --loudness`; measured after the export, the sound
  re-encoded alone with the gain and the limiter (`loudness::bring_to`).
  The limiter's −1.2 dBFS holds sample peaks; true peak may land ~0.2 dB
  higher (as the limiter note says), under −1 dBTP in the tests so far.
- **Two sound effects at once** (46e): a second SFX lane (gap 3's «not
  yet»). **Built 2026-11-01** (`aee68c1`), confirmed by the user:
  `SoundTrack::Sfx2`; an SFX added over one already playing goes there by
  itself; «Move to SFX 2 / SFX» in the menu. The lane always shows, empty
  or not (rows are fixed per track).
- **The duck's depth** (46f): **built 2026-11-02** (`1cbbd13`, `9167d66`),
  confirmed by the user the same day. The default is now 18 dB (was 12; the guides
  give 15–20, WCAG 20). Measured on the test project with `rosmo360
  mixcheck` (which now prints every speech stretch's gap, not only the
  failing ones) after a duck at each depth. The music under the voice:

  | Depth | 5.7–11.7 s | 12.9–25.9 s |
  |---|---|---|
  | none | 14.2 dB | 2.8 dB |
  | 12 | 26.2 | 14.8 (fails the check's 15) |
  | 15 | 29.2 | 17.8 |
  | 18 | 32.2 | 20.8 |
  | 20 | 34.2 | 22.8 |

  The duck lowers from the line the music already has, so the gap is the
  music's own distance plus the depth (the second stretch's music had
  been raised to 0 dB by hand). With music as loud as the voice the gap
  is the depth itself: 15 would sit on the check's edge, 18 clears it by
  3 dB. A settings.json that already holds a depth keeps it (the slider
  sets it, 3–30).
  The user's check (2026-11-02): the slider was squeezed beside the
  button (`a63143a`: now across the panel), and 20 then 18 lowered the
  music 38 dB. Their rule, for every operation on sound: **start from
  the original, never from the last result; a fix after it is by hand,
  with the level.** So a clip remembers its duck (`ducked`) and the next
  takes it out first (`c25cc9f`); «Lower for voice-over» sets the normal
  level −12 dB instead of −12 from where it is (`0fcd9a3`). Normalize,
  «Back to normal level» and «Continue the level» already started from
  the file or its neighbour.

### 47. The sound menus in groups — idea (user, 2026-11-01)

User, after 46c/46d (confirmed): "group the action buttons in the sound
lanes' context menus into specific groups". The menu on a cut's sound or
a sound clip has grown to ten items in one list. Proposed: sections with
a thin rule and a small dim heading each:
- **Level:** lower for voice-over, back to normal, mute, normalize…;
- **Volume line:** show it, continue the level of the one before, back to
  the original;
- **Clean up:** even out, cut the low rumble, clean up steady noise;
- **Music** (songs only): lower the music under speech;
- **Info.**
Items that toggle show their state (a tick) instead of changing their
words. The lane's own menu (empty stretch) and the point and line menus
keep their one or two items.

### 48. Insta360 clips — idea, very far off (user, 2026-11-02)

Asked about after «what format does our 360 stitch?» (DJI Osmo 360 `.OSV`
only). User: "write it down as an idea for the future, but a very distant
one". Not planned: the user's cameras are DJI (the decision of 2026-10-22).

What it would take, as known on 2026-11-02 (not checked against the
vendor's current terms):
- **The files:** `.insv` video (MP4 with the two fisheye pictures; in some
  modes one file per lens, `_00_` and `_10_`), `.insp` photos; gyro,
  calibration and exposure written in a trailer at the end of the file.
- **Insta360's Media SDK** (Linux too) is given on application, closed and
  not freely redistributable: against the project's rule «no vendor SDKs or
  registrations», so not that way.
- **Without it:** telemetry-parser (which reads the DJI telemetry here)
  already reads Insta360's gyro, as Gyroflow does. The lens calibration
  isn't openly described: it would have to be read out of the trailer or
  fitted from footage, the large part of the work (for DJI it comes ready
  in the file).
- Needs the camera or sample clips first.

### 49. Several angles from one 360 clip, cut between like a multicam — idea (user, 2026-11-02); part of gap 53 (53c)

User: "a multi-shot cut of a 360 video. Different angles, to arrange on the
timeline and cut between. The video row can be collapsible for a 360
video: an inner editor-reframer, where we either pick one scene, or cut
several scenes and with «selections» on the scenes' different rows choose
what is shown on screen now; the sound under them is one, from the
original."

As multicam editing in Premiere or Final Cut, with the cameras virtual:
one sphere, several views of it.
- **The inner reframer.** A 360 cut's row folds open (▸, as gap 39's
  groups) into its angles: rows A, B, C…, each a virtual camera with its
  own view keyframes (yaw, pitch, FOV, projection; following the stick or
  a tracked subject, gap 37). Folded, the cut looks as it does now.
- **Choosing what shows.** Select a stretch on an angle's row and that
  angle is on screen there; the stretches together are the cut's switches
  (a row of coloured stretches above the angles shows which angle plays
  when). A switch is a straight cut, or a 360 transition (a whip pan from
  one angle to the next, gap 25e).
- **One sound.** The clip's own sound plays straight through, untouched by
  the switches.
- **Later:** switching live while it plays (keys 1, 2, 3…, as multicam);
  an angle per detected face or person (gaps 30, 37).
- **Data, backward compatible:** a cut's optional `angles` (name and
  keyframes each) and `switches` (clip time, angle); the cut's keyframes
  as they are now are angle A, so old projects open unchanged. The player
  and both exports take the camera from the angle playing at each frame;
  CPU and GPU alike.

### 50. A readable timeline: a time ruler, main lanes first, folding lanes — done, confirmed by the user 2026-11-02

User: "the ruler is unreadable, there are no seconds on it, and it sits
under «Effects». Separate it better, by best practice. Video and its clip
sound are the main lanes. Text and Overlay collapsible, folded by
default; SFX 2 collapsible too. A bigger ruler with time labels, and a
choice in Settings: compact (as now) or large."

What editors do (checked 2026-11-02): the ruler is always on top and
always visible, a press or drag on it moves the playhead (IMG.LY,
Premiere, Resolve); lanes fold to a thin strip with a ▸ (Kdenlive,
Resolve's track height; Shift folds all of a kind in Kdenlive); headers
stay compact; the video group and the sound group are set apart
(RailCut's sections).
- **50a, ruler — done (`9b0e7ef`).** A ruler canvas over every lane:
  minor ticks and labelled seconds (`1:05`, `1:05.5` when zoomed in,
  80 px apart at least at any zoom); the playhead head on it drags to
  scrub; mix-check findings marked at its foot. Settings › Timeline
  «Large time ruler» (28 px, the default) or compact (16 px). The old
  seek bar is a thin rule under the keyframe rows.
- **50b, Text and Overlay fold — done (`fd4cfe7`).** Each folds to an
  18 px strip (`FOLDED`) showing its items as bars and how many
  («▸ Text (2)»); a press opens it, its open name «▾ Text» folds it.
  Folded by default, kept in settings.json. Adding text or overlay
  pictures opens the lane, so new work is never hidden.
- **50c, SFX 2 folds — done (`fd4cfe7`).** The same strip among the
  sound lanes (`Rows::folded`, `lane_h`); its S/M buttons still work
  folded; a sound overflowing into SFX 2 opens it. Folded lanes take no
  fades, points, level tags or line presses.
- **50d, groups — done (`72b6ce3`).** A gap and a faint heading between the picture group
  (Text, Overlay, cuts, clip sound, key rows) and the sound group
  (Voice, Music, SFX, SFX 2), so the eye finds the main lanes at once:
  16 px, a faint SOUND and a thin rule.
- Later: Shift+press folds every foldable lane (Kdenlive); a lane's
  height dragged by its lower edge.

### 51. The exporter grown up: 4K and 8K, codecs, bitrates, presets — idea (user, 2026-11-02)

User, after asking whether the editor uses the GPU yet: "write down what
you propose, and keep in mind we haven't developed the exporter yet: it
will need 4K and 8K and different bitrates and so on". Milestone
«Експорт».

**Today (code facts, 2026-11-02).** `export.rs`'s `Resolution` is 720p,
1080p, 1440p, 2160p (the short side) in the aspect lock's shape, or an
exact `--size`. Two formats (`ffmpeg::Format`): MP4 = libx264 CRF 18,
preset medium, yuv420p, AAC at ffmpeg's default bitrate; Resolve =
10-bit DNxHR HQX + 16-bit PCM in MOV. Clips export with handles and
Graded; the loudness target (46a). No bitrate, codec, frame rate or
audio choice; no hardware encoder; no HDR or 360 output. Speed on the
RTX 3090 from the full OSV: ~21 fps at 1080p 8-bit, ~15 fps 10-bit
(gap 52 says why).

**Proposed, in parts:**
- **51a Sizes.** 2880p and 4320p (8K) beside today's; for a 360
  export, an equirect 7680×3840 (YouTube's 8K 360). The Osmo's lenses are
  3840² each, so a framed 8K is upscaled: say so in the panel, while an
  8K equirect uses the sphere's whole detail.
- **51b Codecs.** H.264 (now), H.265/HEVC 8 and 10-bit (half the size for
  the look, the codec for 4K/8K and HDR), AV1 (SVT-AV1: smallest, slow on
  the CPU); keep DNxHR for Resolve; ProRes (`prores_ks`) for a Mac
  editor. H.264 stops at about 8192×4320 (level 6.x): 8K should default
  to HEVC.
- **51c Hardware encoders.** NVENC (H.264, HEVC, AV1 on newer cards),
  VA-API, VideoToolbox on the Mac, probed like the decoders
  (`Hwaccel::works`) with x264/x265 as the fallback. At 1080p NVENC
  gained nothing (x264 used idle cores); at 4K and 8K the CPU encoder
  becomes the limit, so measure there first.
- **51d How big: quality or bitrate.** By default quality (CRF / CQ),
  in a few named steps; or a target bitrate in Mbps with two passes
  (x264/x265), and a cap for players that need one. Platform presets
  carry their recommended bitrates (YouTube's page for SDR/HDR by size
  and frame rate; check the numbers when building).
- **51e Frame rate.** The edit's (now), or 24/25/30/50/60; a 100 fps
  clip rendered at 30 runs ~3× faster (PR #5's idea).
- **51f Sound.** AAC at 192/256/320 kbps (now ffmpeg's default), 48 kHz;
  PCM where the container takes it.
- **51g HDR out.** HEVC 10-bit HLG or PQ, tagged, for HDR phone clips
  and log footage, instead of always tone-mapping to SDR (gap later; it
  needs a colour pipeline that stays HDR).
- **51h 360 out.** The whole sphere as equirect with spherical metadata
  (left over from upstream's timeline polish), for YouTube/VR players.
- **51i Presets.** Named sets in the Export panel and settings (YouTube
  4K, Instagram 9:16 1080, Resolve, Archive HEVC 10-bit), with the file
  size and time it will take shown before Export.

Order when it comes: 51d + 51f (cheap, every export), 51b HEVC, 51a 4K
and 8K with 51c measured, 51i presets, then 51e, 51h, 51g. Gap 52 decides
how fast 4K and 8K are.

### 52. The frames' fast path through the GPU (zero-copy) — idea, the main speed work (2026-11-02)

From the user's question «does the editor use the GPU yet?». The answer:
decoding (CUDA / Vulkan Video / VA-API through ffmpeg), the stitch,
stabilisation, rolling shutter, grade, LUT, HDR→SDR and dither are on
the GPU (wgpu); the encoder (x264), the sound, the seam measure, the
thumbnails and contact sheets are on the CPU. What limits it is neither:
each frame travels GPU (decoder) → ffmpeg's download → a pipe → our
memory → upload → GPU (stitch) → readback → pipe → x264. At 10 bits
moving the bytes, not computing, holds a full-res 1080p export to ~15
fps against ~21 (measured 2026-10-14, `CLAUDE.md` «Not done yet» 1).
At 4K and 8K (gap 51) it gets worse.

**Proposed, cheapest first:**
- **52a Smaller readback.** Convert RGBA to NV12 / P010 in a shader
  before reading the frame back: 1.5 or 3 bytes a pixel instead of 4 or
  8, and no RGB→YUV in ffmpeg. Export side only, no new dependency.
  Measure first: GPU vs CPU export at 1080p and 4K.
- **52b A Linux prototype of zero-copy decode.** One OSV decoded
  in-process (ffmpeg's libraries instead of the program) into the
  shader, sharing the decoder's memory with wgpu's device (CUDA or
  Vulkan Video on Linux). Measure the gain before building it per
  platform (VideoToolbox's IOSurface on the Mac, D3D11 on Windows).
  Brings back the licence question (linking ffmpeg: LGPL or GPL).
- **52c Encode on the GPU from the GPU.** With 51c's NVENC, hand the
  rendered frame to the encoder without the readback (CUDA interop).
  Last, and only if 52a and 52b leave the encoder waiting.

Unmeasured guess (`CLAUDE.md`): export 30–40 fps at either depth at
1080p; full-res playback on the dev laptop.

### 53. Framing grown up: best practices, faces, tracking, angles — idea, large (user, 2026-11-02)

User, asked whether gap 10 (the Edit / Framing modes) counts as
confirmed: "framing has to inherit best practices, and add face
recognition, object tracking and so on (the multicam we described can
hang here too)". So gap 10 is the base, not the end; milestone
«Кадрування».

**What there is (code facts).** The Framing switch (gap 10) with free
look, view keyframes and their eases, stabilisation modes, projections
(perspective to little planet, typed angles), Follow: the selfie stick
(from the attitude) or a colour track (`rosmo360 track`, `src/track.rs`);
ordinary videos framed by pan and zoom (`view::Crop`). Every view change
is an op (`KeyOp::View`/`Follow`, OPS-API), so whatever comes next can
key the view the same way.

**First: research the best practices** (a written page, as
`TIMELINE-UX.md` was for the timeline): how Insta360 Studio (Deep Track,
its keyframe panel, «ViewFinder»), DJI Studio / LightCut, GoPro Player's
reframe, Premiere's Auto Reframe, Final Cut's Smart Conform and
Resolve's Smart Reframe frame a shot: what they key, how a subject is
picked and corrected, how a 360 shot becomes a flat one, what users
praise and curse. Then the parts below get ordered from it.

**The parts, as they stand:**
- **53a Faces and people.** Detect faces and bodies in the sphere (and in
  flat videos); a detected person is something to follow, as the stick
  and a colour are. footage-agent's gap 30 (who is in the frame, names
  from Immich) can name them. Needs a model: an ONNX face/person detector
  run in-process or by footage-agent; YOLO's AGPL is ruled out (`CLAUDE.md`
  0.2.8), so a permissively licensed model.
- **53b Object tracking** (gap 37, moved here): a box drawn on anything,
  followed frame by frame, keys the view (or an overlay); corrected by
  hand along its path.
- **53c Angles as a multicam** (gap 49, moved here): one 360 clip, several
  virtual cameras, cut between on the timeline; an angle per detected
  person later (53a).
- **53d Framing aids** to be taken from the research: rule-of-thirds and
  safe-area guides for the aspect, a horizon line, a «look room» hint,
  smoothing a hand-keyed path.

### 54. A header column for the lanes: what a whole lane does, at a glance — done 2026-11-04 (`c6ee4b6`, `8ec3df3`), confirmed by the user

User, after asking why the music's waveform showed a bright wave inside
a darker, bigger one (the file and the heard sound, gap 33d: the Music
lane's own level was −6 dB in the Audio panel while the clip read 0 dB):
"add plain information about the whole lane; maybe keep about a
centimetre on the left of the timelines for the main facts, and a click
shows all of the lane's own settings, so there's no confusion".

**Today.** Each sound lane has a tag floating over its left end («Music»,
S, M, folded ▸/▾; gap 46d, 50b), drawn over whatever clip is there. The
lane's own level lives only in the Audio panel; the clips' tags show
their own level (gap 44). So a clip at «0 dB» can sound 6 dB down with
nothing on the lane saying why.

**Proposed** (as DAWs and Resolve do: track headers):
- A fixed column (~1 cm, about 40–60 px at 100 %, with the UI scale)
  left of every lane, picture and sound alike, that doesn't scroll with
  the timeline: the lane's name, its level when it isn't 0 dB («−6 dB»),
  S and M, fold ▸/▾; for sound lanes a small level bar after the mix
  (from the meter, gap 46b's LUFS when measured).
- A click opens the lane's own settings in one place: its level (the
  Audio panel's slider), solo/mute, normalize every clip on it, auto-duck
  (music), «what the mix check says about this lane» (gap 45), fold.
- The floating tags go: the column holds them, and no clip is covered.
- Picture lanes get the same header (cuts, overlay, text: name, fold,
  what's on).

Decided (user, 2026-11-02): the ruler (gap 50a) starts after the column.
Its width: 1–2 cm, to try out; not fixed to the user's 43-inch monitor,
as they won't always edit on it, so it follows the UI scale and may be
narrowed (or dragged) on a small screen.

**Built 2026-11-04** (`c6ee4b6`): `timeline::HEAD`, 128 px (to fit «Clip
sound» with S and M at a readable size; one constant to tune), on every
canvas of the timeline (lanes, overlay and text lanes, the markers' row,
the ruler); time starts after it, and each canvas paints the column last
so scrolled clips go under it. In it: the lanes' names (Video, Clip
sound, Effects / View / Motion, SOUND, each sound lane, Overlay, Text,
Markers), S and M, folds, and a tall lane's own level under its name
when not 0 dB («lane −6.0 dB»). A click or right-click on a sound lane's
header opens its menu: the lane's level slider and what acts on the
whole lane (normalize every clip, the music's duck); the clip sound's
header: its level and «Normalize every cut». Each lane's cell is
tinted in its colour with a rule under it (`8ec3df3`, the user's ask on
seeing it). Left: a level bar from the
meter per lane, the mix check's verdict per lane, dragging the column's
width, picture lanes' menus.

### 55. Moving the playhead by keys, in steps that make sense — small (user, 2026-11-02)

User, after checking pack 2: "add the playhead line's movement too: now
the left/right keys just throw the playhead back and forth in big steps
(it isn't clear what they do)". Today ← / → seek ±5 s (`keys.rs`), with
nothing saying how far; `,` `.` step a frame; Shift+← / → turn the view
(framing only); ↑ / ↓ pitch (framing only).

Proposed for editing mode (framing keeps its keys), as Premiere and FCP:
- ← / → one frame (as `,` `.` are now);
- Shift+← / → one second (or ten frames);
- ↑ / ↓ the previous / next join (a cut's edge, a sound clip's or an
  item's edge, later a marker);
- Home / End the edit's start and end;
- the step shown by the playhead for a moment («−1 f», «+1 s», «join»),
  as the drag readout is.
**Built 2026-11-03** (`915239d`), confirmed by the user the same day
(after the fixes under gap 5), with these defaults: the 5 s jump is gone (Alt+← / → move a cut since pack
2); Shift's step is one second; ↑ / ↓ and Home only in editing (framing
keeps pitch and the view's reset), End in editing too; a field of the
angles strip being typed in keeps ↑ / ↓.

### 56. A window of every shortcut, grouped — small (user, 2026-11-03)

User, to check gap 55 and the rest at once: "make a separate element
that opens a help with the shortcuts, grouped". **Built 2026-11-03**;
its button checked by the user, ? / F1 to check on a wired keyboard: ? or F1, or a keyboard button beside
Settings, opens a window over the editor (`player/shortcuts.rs`): play
and the playhead, cuts, the timeline's mouse gestures, sound, framing,
panels and files. Esc, ?, F1 or a click outside closes it. Hidden until
asked for, so the «no help text in the UI» rule holds. Its list is kept
in step with `keys.rs` by hand: a key added there is added here.

### 57. Cutting footage to music's beat, for the AI edit — idea, large (user, 2026-11-03)

User: "an idea for the AI-editing API: sometimes there's video that can
be cut to music. It needs to find the beats (as in a sampler) and have
several ways of cutting to them (in a row, separate scenes, and so on),
held on something interesting. An AI model could categorise what's seen
in the frames better; as it runs locally for me, the GPU can be used.
For a mobile version that can be left out."

Builds on gap 13 (beats found, cuts snapped to them), the ops API
(`OPS-API.md`: a coarse op expanding into plain `CutOp`s, one undo step,
previewable with `apply` without `-o`), and footage-agent's frame
understanding (gaps 29, 30). Milestones: the beats in «Ритм і переходи»,
the cutting in «ШІ-монтаж».

- **57a Beats, as a sampler finds them**: onsets, the tempo, the beat
  and bar grid, downbeats and stronger hits (drops), with a confidence;
  measured once per song and cached like waveforms. Shown on the music
  lane (gap 13), and a query for the AI (`beats` beside `describe`).
- **57b Ways of cutting to them** (each a coarse op the user or the AI
  picks, with its numbers):
  - in a row: one clip cut on every beat (or every 2nd, 4th, bar), its
    pieces in order, a fast montage;
  - separate scenes: one shot per beat or bar from several clips;
  - by intensity: longer shots in quiet bars, quicker on the drop;
  - a held moment: a shot that lasts across beats where something
    happens.
- **57c Held on something interesting**: each shot's in point chosen
  where the picture is worth it: motion and faces from what exists
  (`scan`'s turning and shake, colour track, gap 53's people), and an
  optional vision model categorising frames (people, landscape, action,
  food, a view…) and scoring them. Local, on the GPU (CUDA / Vulkan):
  the user's machine has one. Kept out of a mobile build (gap 40), whose
  cutting uses beats and the simple measures only.
- **57d The result stays editable**: plain cuts on the timeline, each
  op's summary saying why («on beat 17, a face»), one undo step.

Open when designing: which beat tracker (a Rust onset detector, or
footage-agent's Python with librosa / madmom / aubio); which vision model
(licence: permissive only, as for 53a); how the AI picks among the ways.

### 58. Select the cut under the playhead by a key — small (user, 2026-11-03)

User: "I wind the playhead down to some cut and want to move it with
Alt+←, but another cut is selected at the time. Some hotkey?" **Built
2026-11-03**, confirmed by the user the same day (`eefa803`): D (Premiere's key)
selects the cut under the playhead, the one shown through a crossfade;
the playhead says «selected». So: ↓ to a cut, D, Alt+← / →.

### 59. The bars' buttons grouped — small (user, 2026-11-03)

User: the buttons are spread all over the window; group them, the global
ones at the top, the timeline's at the bottom, where the row has the whole
width; groups that can be shown collapsed (buttons split by groups) or
opened (a little wider, the groups' names shown). **Done 2026-11-03**
(`93ef86b`), confirmed by the user ("fits for now"; they will tune the
layout during real editing):
- Top bar, global: App (settings, shortcuts) | Project (new, open, save,
  save as) | Export (aspect, export) | 360 view (while framing a 360 cut)
  … Mix (meter) | Window (interface size, fullscreen) | status.
- Transport row, the timeline: Play (play, time, save frame) | Add at the
  playhead (clip, blank, music, SFX, voice, overlay, text) | Cut (split,
  delete, crossfade) | History | Look (effects) | Mode (Framing, Mix
  check) … Timeline zoom | Sound.
- Framing row (only while framing): Keyframe (key, delete, ease, reset,
  follow, colour track, save sphere) | Angles (the typed strip; its FOV
  field replaces the slider, which comes back when Settings hides the
  strip).
- A chevron at the transport row's start shows or hides every group's
  name above its buttons (`settings.json` `bar_labels`, off by default).

### 60. The bin as a view of its own, in tiles — idea (user, 2026-11-03)

User: "a separate view, instead of the playback screen: an edit bin. The
project is made as folders from the start, so it would be logical to have
it all in tiles. Click «Edit bin» and a window opens where, by drag and
drop or a click and a context menu, you add or remove resources."

What it would be:
- A button (and a key) swaps the picture for the bin, as Mix check
  already stands in the picture's place; the timeline stays below, so a
  tile can be put on it.
- Tiles by the project's folders (Footage, External video, Music,
  Voice-over, Sound effects, Graphics, LUTs), big thumbnails with a size
  slider, the length on each; the skimmer and ratings of gap 2 belong
  here.
- **Add**: drop files from the file manager onto the view (iced gives the
  window's dropped files) or a «+» per folder; each file is copied into
  the folder its kind belongs to (originals, like the Kos archive, only
  read, never moved).
- **Remove** (decided by the user, 2026-11-03): from the right-click menu,
  to the system's trash, asked first, and only for a file inside the
  project's folder (a copy). Raw material outside it (the Kos archive, a
  card) is never trashed: the menu only takes it out of the bin. A file
  the edit still uses can't be removed (the menu says which cuts use it).
  Also in the menu: add at the end / at the playhead / at the join (the
  bin's three buttons today), Info, Show in folder.
- No drag from a tile onto the timeline (user, 2026-11-03): the add
  buttons do it, and most projects come already cut in footage-agent's
  contact sheet.

It grows the narrow bin panel (gap 2) rather than replacing it: the panel
for quick adding while cutting, the view for sorting the material. Joins
the «Перегляд з біна у вікні Omnix» card: the view where a tile is
previewed is the same place.

### 61. The contact sheet standalone, for DeX and tablets — far idea (user, 2026-11-03)

User: one of the far ideas, footage-agent's contact sheet as a standalone
app, so it can be used on Android DeX (the S26 Ultra workstation) or a
tablet. Most projects are pre-cut there, so choosing and noting shots away
from the PC is worth having.

To find out when it comes up: what the sheet needs from the PC today
(the clips' frames and proxies, `notes.json`), whether it can carry them
along (a project folder copied or synced to the device) and write
`notes.json` back for the PC to pick up; and the form: a web page served
by the PC over Tailscale is the cheapest, an installable web app (PWA)
works offline. Milestone «Інші платформи».

### 62. Elements tied to their cut: sounds, music, voice-over, text and overlays move with it — idea (user, 2026-11-03)

User: "group the sounds, music, voice-over, text and overlay with the
video, so that once I've cut effects and music to it and drag the video
along the timeline, its own elements move with it. They need ids of
their own, since the same sound can be on the timeline many times under
different videos, and so they aren't dragged by accident."

Final Cut calls these connected clips; CapCut links them to the main
track. What we have for it: every cut, sound clip and overlay has an id
in the project since the ops API (`OPS-API.md`), so an element can name
the cut it belongs to (an optional `tied_to: cut id`, kept relative: its
offset from the cut's start). Moving, trimming away or deleting the cut
then moves or asks about its elements; by id, so the same bell under
two cuts belongs to each separately. The link shown on the timeline (a
thin line or a corner mark to its cut), made and broken from the menu
or a key, never by a drag alone. Designed with gap 4's connected clips
and gap 8 (several selected), as one format change. Milestone «B-roll і
групи».

### 63. Quick copies of ordinary videos for the playhead — done 2026-11-04 (`3c06119`), confirmed by the user ("smoother, not perfect": the rest is gap 65)

User (checking pack 3's snapping): dragging the playhead lagged and a
core ran at 100 %. Measured: the test clips (phone-like H.264 1080p)
have a keyframe every 8 s, so each jump of a paused playhead decoded up
to 240 full frames (0.43 s, ~8 cores at 7.9 s into talk.mp4). The user
had asked earlier for preview "from a proxy".

Built: `player/scrub.rs`. An ordinary video, once opened, gets a copy in
the clip cache (`<key>.scrub`, a kind of entry `cache move` / `clear`
handle): 960 px, a keyframe every 6 frames, no sound, x264 ultrafast, as
stored (turned and its colours handled on decoding, as the file's);
about a second and a third of the size for the test clips. Paused frames
(dragging the playhead, stepping by keys) decode from it in software:
0.05 s a jump. The full frame still comes from the file once the
playhead rests. Pictures get none; 360 clips have their LRF.

Left: a copy is made only while the cache is on; long clips take longer
(roughly 1/15 of their length on the dev PC); no progress shown.

### 64. The preview's size as a setting — idea (user, 2026-11-04)

Today an ordinary video plays at 1920 px on its long side at most
(`playback::FLAT_PLAY`), moves of the playhead show its 960 px copy (gap
63), and a rested frame is full size if the file is bigger; a 360 clip
plays its LRF (or the OSV at the Source picker's size). Idea: a Settings
choice for the playing size of ordinary videos, e.g. 720p / 1080p /
full, so a weak machine or a laptop plays smoothly (1080p stays the
default), maybe with the scrub copy's size beside it. Kept in
`settings.json`. Milestone «Інші платформи» (the laptop, the tablet).

### 65. The player's work moved onto the GPU, with a setting for machines without one — idea (user, 2026-11-04)

User, after gap 63: "smoother, not perfect. The graphics card sits idle
while everything is ground on the CPU — for nothing. Make it a setting
too (for when the program moves to a laptop without a dedicated card)."

Where the player's CPU goes now, to measure before choosing:
- ordinary videos' paused frames decode from the scrub copy in software
  (gap 63: chosen because a GPU decoder takes 0.5-1 s to start, longer
  than the copy's whole jump); playing, the file decodes on the GPU where
  the probe found one, else in software;
- every frame is downloaded from the GPU, scaled and converted on the
  CPU, piped, and uploaded again (gap 52, zero-copy, is the cure);
- thumbnails, waveforms, the seam measure, the sound are on the CPU.

Ideas: keep one GPU decoder open and seek it instead of starting one per
jump; scale and convert on the GPU (`scale_cuda` / `scale_vulkan`, which
were slower once, measure again); then gap 52. A Settings choice
«Decoding: automatic / GPU / CPU» (the CLI has `--hwaccel` already; the
top bar's status icon says which is used), automatic picking the GPU
when there is one and staying light on an integrated one (the laptop,
gap 40). Goes with gap 64 (the preview's size). Milestone «Експорт»
(the GPU fast path).

### 66. The button groups in colours of their own — idea (user, 2026-11-04)

User: "different colours for the groups of buttons for editing and for
navigation, so what's needed is easier to find by colour patterns in the
UI (now the buttons are all blue)". Builds on gap 59 (the bars grouped)
and gap 43's palette proposal (`UI-COLOUR.md`, «Omnix Neutral» and the
lanes' colours): a muted tint per group, e.g. adding at the playhead,
cutting, history, playing and moving about, framing, panels, each the
same wherever it appears (the shortcuts window's groups too), and never
colour alone (icons and the groups' names stay). Milestone «Колір і
текст» with the palette card.

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
6. **Кадрування** (gap 53, user 2026-11-02): framing grown up from gap
   10's switch: research the best practices first (Insta360 Studio, DJI
   Studio, GoPro, Auto/Smart Reframe), then faces and people (53a, with
   footage-agent's 30), object tracking (37 → 53b), angles as a multicam
   (49 → 53c), framing aids. After B-roll and groups, as angles change the
   format beside groups; reordered freely.
7. **Ритм і переходи**: 13 beats → 25a basic transitions with quick
   placement → 25d stingers («перебивки») → 25c templates → 25b edge
   transitions → 25e 360 looks. Stingers come early because the user
   already has stinger clips (film burn, white noise).
8. **Колір і текст**: 14 levels → 15 bars → 26 glow/blur → 6 vignette shapes →
   16 subtitles (needs 29) → text-based editing (needs 29).
9. **Швидкість і автоматизація** (gap 31): constant speed per cut with
   its sound following (atempo), measured for drift → speed ramps →
   automation lanes per value (the user's FL Studio idea). Large and
   careful; after B-roll and groups, since it touches the same time
   mapping. With it: 31b keep-the-pitch switch, 31e speed points on the
   main timeline, 31c the speed editor window, 31d right-click keyframes,
   and the automation idea that 37 object tracking (now in «Кадрування»)
   keys into.
10. **Якість 360** (from `OPENOSV.md`, mostly needs 360 footage: one public
   sample now, the user's camera ~2027-02): 17 easing presets and 18 bundled
   LUT any time (small, useful now) → 24 file facts → 19 lens protectors →
   20 mask release → 21 tilt diagnostic → 22 sky seam field (patents first)
   → 23 DJI presets → Osmo 360 II.
11. **Експорт** (gaps 51, 52; user, 2026-11-02): what the export makes
   (4K and 8K, HEVC and AV1, bitrates, frame rates, sound, presets, HDR
   and 360 out) and how fast (the frames' path through the GPU). Before
   the AI edit, which needs it; reordered freely.
12. **ШІ-монтаж** (gap 28, `AUTO-EDIT.md`): the user's main wish, but put
   last on purpose (user, 2026-10-23: "I wouldn't do AI functions yet:
   Omnix doesn't have its full working functionality"). The design stays
   ready. The pieces it needs arrive with the milestones above.
13. **Інші платформи** (gap 40, user's question 2026-11-01): Mac and
   Windows realistic (upstream builds both), Android a new UI over the
   same core. Not planned; no new Linux-only code without a stub.
