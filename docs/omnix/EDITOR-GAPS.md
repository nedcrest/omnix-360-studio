# Editor gaps — from the first real edit

The user's first test, 2026-10-22: the Kos 22.08 project made by
footage-agent's «→ Omnix» button opened, its clips are in order and play.
What they missed while editing, what the code already has, and how to fix
each. Since then every new idea lands here too, numbered in arrival order.
The numbers are identities, not priorities; the order of work is the
milestones at the bottom (and `ROADMAP.md`'s `віха` field). Finished gaps
live in `EDITOR-GAPS-DONE.md`.

## Index

Every gap by number. The open ones (ideas, planned, partly built) are
below; the finished ones moved to `EDITOR-GAPS-DONE.md` (2026-11-04), same
numbers, so a number named anywhere is found in one of the two. A gap
done in part stays here until its last card is done.

| # | Gap | Where |
|---|---|---|
| 1 | Insert a clip between two cuts | [done](EDITOR-GAPS-DONE.md) |
| 2 | A bin (project library panel), like Kdenlive's | below |
| 3a | Cleaned voice in place of a clip's sound | [done](EDITOR-GAPS-DONE.md) |
| 3 | Sound lanes | [done](EDITOR-GAPS-DONE.md) |
| 4 | The clip's sound as its own lane | below |
| 5 | Markers | [done](EDITOR-GAPS-DONE.md) |
| 6 | Vignette shapes | [done](EDITOR-GAPS-DONE.md) |
| 7 | A wider selection frame | [done](EDITOR-GAPS-DONE.md) |
| 8 | Select several cuts, act on them together | below |
| 9 | Timeline ease and accessibility | below |
| 10 | Two modes: Edit and Framing | [done](EDITOR-GAPS-DONE.md) |
| 11 | Bring a speech clip up to −16 LUFS | [done](EDITOR-GAPS-DONE.md) |
| 12 | True-peak limit on export | [done](EDITOR-GAPS-DONE.md) |
| 13 | Cuts on the beat | below |
| 14 | Black and white levels | [done](EDITOR-GAPS-DONE.md) |
| 15 | Cinematic bars | [done](EDITOR-GAPS-DONE.md) |
| 16 | Subtitles from speech | below |
| 17–24 | From OpenOSV | below |
| 25 | Transitions and stingers | below |
| 26 | Glow and blur | [done](EDITOR-GAPS-DONE.md) |
| 27 | Overview strip and centred playhead | below |
| 28 | AI auto-edit from the contact sheet | below |
| 29 | Speech transcription | below |
| 30 | Who is in the frame | below |
| 31 | Speed: slow motion and fast forward, with automation | below |
| 31b–31e | Speed: the pitch choice, a speed editor window, keyframes by right-click | below |
| 32 | A blank stretch on the main lane | [done](EDITOR-GAPS-DONE.md) |
| 33 | Right-click menu on a cut's sound | [done](EDITOR-GAPS-DONE.md) |
| 34 | Import from devices in the studio | below |
| 35 | Code analysis and refactor: scalable, API-driven, documented (user, 2026-10-28) | [done](EDITOR-GAPS-DONE.md) |
| 33b | Show a cut's volume line from the menu | [done](EDITOR-GAPS-DONE.md) |
| 33c | Fade handles on a cut's own sound | [done](EDITOR-GAPS-DONE.md) |
| 33e | A sound clip's own volume line, apart from the track's | [done](EDITOR-GAPS-DONE.md) |
| 33f | Normalize and even out sound clips, the voice-over above all | [done](EDITOR-GAPS-DONE.md) |
| 33g | Removing points and lines, and the menu where the pointer is | [done](EDITOR-GAPS-DONE.md) |
| 33h | Bending a line between two points into a curve | below |
| 33i | No track volume line: each clip's own line only | [done](EDITOR-GAPS-DONE.md) |
| 33d | Waveforms drawn as they're heard | [done](EDITOR-GAPS-DONE.md) |
| 36 | Record a voice-over inside Omnix | [done](EDITOR-GAPS-DONE.md) |
| 37 | Object tracking | below |
| 38 | A slider dragged during playback makes a key per frame | [done](EDITOR-GAPS-DONE.md) |
| 39 | Automation lanes: Effects and View as folding groups of curves | below |
| 40 | Other platforms: Mac, Windows, an Android tablet or phone | below |
| 41 | Core, functions and API: what to unify and separate | below |
| 42 | Unit and integration tests | [done](EDITOR-GAPS-DONE.md) |
| 43 | Editing-software best practices, colours, a switchable UI palette | below |
| 44 | A clip's overall level, by hand: a knob after the duck | [done](EDITOR-GAPS-DONE.md) |
| 45 | A mix check: the sound's levels against a platform's, drawn and clickable | [done](EDITOR-GAPS-DONE.md) |
| 46 | More for the sound: what the review found | [done](EDITOR-GAPS-DONE.md) |
| 47 | The sound menus in groups | [done](EDITOR-GAPS-DONE.md) |
| 48 | Insta360 clips | below |
| 49 | Several angles from one 360 clip, cut between like a multicam | below |
| 50 | A readable timeline: a time ruler, main lanes first, folding lanes | [done](EDITOR-GAPS-DONE.md) |
| 51 | The exporter grown up: 4K and 8K, codecs, bitrates, presets | below |
| 52 | The frames' fast path through the GPU (zero-copy) | below |
| 53 | Framing grown up: best practices, faces, tracking, angles | below |
| 54 | A header column for the lanes: what a whole lane does, at a glance | [done](EDITOR-GAPS-DONE.md) |
| 55 | Moving the playhead by keys, in steps that make sense | [done](EDITOR-GAPS-DONE.md) |
| 56 | A window of every shortcut, grouped | [done](EDITOR-GAPS-DONE.md) |
| 57 | Cutting footage to music's beat, for the AI edit | below |
| 58 | Select the cut under the playhead by a key | [done](EDITOR-GAPS-DONE.md) |
| 59 | The bars' buttons grouped | [done](EDITOR-GAPS-DONE.md) |
| 60 | The bin as a view of its own, in tiles | [done](EDITOR-GAPS-DONE.md) |
| 61 | The contact sheet standalone, for DeX and tablets | below |
| 62 | Elements tied to their cut: sounds, music, voice-over, text and overlays move with it | below |
| 63 | Quick copies of ordinary videos for the playhead | [done](EDITOR-GAPS-DONE.md) |
| 64 | The preview's size as a setting | below |
| 65 | The player's work moved onto the GPU, with a setting for machines without one | below |
| 66 | The button groups in colours of their own | [done](EDITOR-GAPS-DONE.md) |
| 67 | Bokeh: the background softened behind a person | below |
| 68 | Curves and colour by hue | below |
| 69 | Scopes: a histogram and a waveform of the picture | [done](EDITOR-GAPS-DONE.md) |
| 70 | The text and overlay lanes as groups of several lanes | [done](EDITOR-GAPS-DONE.md) |
| 71 | Two viewers: the source and the edit; effects tried beside the original, then applied | below |
| 72 | The contact sheet's next round: music hints, ready music, its flow, data for AI | below |
| 73 | Multicam: several cameras as one clip, a module of its own; 360 angles the same way | below |
| 74 | Dailix: the studio and contact sheet as a native Rust app of their own | below |
| 75 | A cut's flags from the contact sheet as markers on the edit | [done](EDITOR-GAPS-DONE.md) |
| 76 | The program's name: rosmo360 → omnix | [done](EDITOR-GAPS-DONE.md) |
| 77 | Dailix: voice clean-up and loudness for camera and phone sound | [done](EDITOR-GAPS-DONE.md) |
| 78 | A vertical video in a wide edit: its sides filled from itself, not black | below |
| 79 | Omnix's own face: the footer's credit and the start picture | [done](EDITOR-GAPS-DONE.md) |
| 80 | Omnix starts at a window of recent and found projects (a projects folder set in both programs); «New project» goes to Dailix | [done](EDITOR-GAPS-DONE.md) |
| 81 | Auto-duck under talk in a cut's own sound, not only cleaned voices | [done](EDITOR-GAPS-DONE.md) |
| 82 | Dailix: global projects (a trip, an event) holding video projects; a home of them with notes and tags; opening where it was closed | [done](EDITOR-GAPS-DONE.md) |
| 83 | Dailix finds where people talk in a clip, even in bad sound, and pulls the voice out | [done](EDITOR-GAPS-DONE.md) |
| 84 | A video drawing on several trips (a year in review) | below |
| 85 | Project cards in a grid, a GIF of the project on hover, basic facts and a description | below |
| 86 | A program of its own: animated cutaways from a prompt and parameters (8-bit game, animated slides) | below |
| 87 | Style looks (old film, VHS, neo-cyberpunk) for a video, an overlay picture or text; animation presets for text and pictures | [done](EDITOR-GAPS-DONE.md) |
| 88 | The Osmo 360 II: its calibration and 8K50 decoding | below |
| 89 | Auto colour: exposure and white balance set from the picture (or by an AI), keyed where the camera misses | [done](EDITOR-GAPS-DONE.md) |
| 101 | Nothing longer than the film: sound clips, text and pictures stop at its end; a warning for those past it | below |
| 100 | Outside material in Dailix: music, sound effects and video by link (YouTube and the like), pictures dragged in or by link and converted, each with its licence noted | below |
| 99 | Sync lock on a sound track: when the picture's edit gets shorter or longer, the music after that point moves with it | below |
| 98 | The Effects panel's extended view: its groups as framed blocks side by side, for a wide monitor | [done](EDITOR-GAPS-DONE.md) |
| 97 | The text and overlay lanes: names, dragging an item between them, hiding one | [done](EDITOR-GAPS-DONE.md) |
| 96 | A timeline that scrolls up and down: a «Scrollable» switch keeps the lanes' sizes, a vertical scroller, Alt+wheel | below |
| 95 | Text: a colour of its own, letter spacing, a box behind it; its colour automatable | below |
| 94 | The Effects and Text panels tidied: text's look in Effects, groups for items (Place, Show, Animate), the film's fades to Transitions, consistent controls | [done](EDITOR-GAPS-DONE.md) |
| 93 | Effects edit the selected clip only, with the playhead on it; a clear notice when they're apart | [done](EDITOR-GAPS-DONE.md) |
| 92 | Play a stretch: drag a range on the ruler, Shift+Space plays it once and comes back to its start, paused; a button beside Play | [done](EDITOR-GAPS-DONE.md) |
| 91 | Pictures from the bin or the file manager dragged onto the overlay lane; a «Pictures» folder in every project | [done](EDITOR-GAPS-DONE.md) |
| 90 | The Effects panel sorted: groups, a side panel for complex controls, one scope | [done](EDITOR-GAPS-DONE.md) |

## Open gaps


### 2. A bin (project library panel), like Kdenlive's — v2 done; the bin view in tiles with its own player and I/O done in gap 60

User: wants a bin, maybe with better UI, with "our extra sound tracks" in it.

**Now (`src/player/bin.rs`, v1 `241f28d`, v2 `e33f65e`):**
- A panel of its own at the far left, beside whichever left panel (Info,
  Settings, Text) is open; folds to a narrow strip (B toggles).
- Pin (🔒) keeps it open, and open at the next start (`Settings.bin_pinned`).
- Lists the project's folder (`bin::root`: the trip of an edit in its
  `videos/<name>/`, folder names by Dailix's `Layout`; above an older
  edit's `04_project`): Footage, External video, Music, Voice-over, Sound effects,
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

**Next for the bin** (2026-11-07: the grid with its size slider, the
player inside Omnix and I/O are done in gap 60's bin view; the hover
skimmer in the bin gives way to the source viewer, 71a):
- E / W from the bin: append / insert at the playhead (needs gap 1);
- drag-and-drop to the timeline (harder in iced);
- sounds to their own lanes (gap 3); LUTs from `09_luts`;
- ratings, notes and tags from Dailix's marking (once `notes.json`);
- the narrow bin panel still previews in an ffplay window: in Omnix, as
  the source viewer (71a). The first note on it — the
  user noticed the separate window (2026-10-22) and expects it in Omnix's
  viewer, like a source monitor in Kdenlive / Resolve. Idea: a preview mode
  that plays the bin's file in the main view (loaded into the library, not
  added to the edit), with its own in/out marks later (add only a part);
  Esc or another click returns to the edit at the playhead.

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

### 8. Select several cuts, act on them together — medium (user, 2026-10-22)

Today one cut is selected (`App::selected: Option<usize>`). Wanted:
Shift/Ctrl+click to select several, then act on all — delete, move, set
effects, and **join** them (e.g. merge cuts of one clip back into one, or
group them so they move together). Needs a selection set instead of one
index, and a choice of what "join" means: merging neighbours from the same
clip is clear; a group of different clips is a format change (a group id).

**Groups with colours** (user, 2026-11-11, after cutting to beats):
a group is any clips — cuts, sound clips, text, pictures — each group
with a colour of its own: its clips framed in it and/or a band of it
behind them on the timeline. A click on one selects the group; delete,
effects, a move where the magnetic edit allows. With sync lock (99) a
group can later mean «these cuts and this song move together». The
format: a group id on cuts, sound clips and items, and the project's
groups (name, colour); after 99.

**Decided with the user (2026-11-11):** a group shows as a frame in its
colour round each of its clips and a faint band of it behind them; it
selects and moves together — a click selects the whole group (Alt+click
one clip), Delete removes it all, its sound clips, text and pictures move
together when one is dragged; its cuts stay where the magnetic edit puts
them. Made as Premiere does: Shift+click adds a clip to the selection
(Ctrl+click already puts a volume point on a sound), Ctrl+G groups,
Ctrl+Shift+G ungroups. Steps: (1) the core — group ids,
the project's groups, ops, `describe`; (2) Shift+click selecting several;
(3) the frame and band; (4) together — select, delete, move.

**Built 2026-11-11, waits for the user's check:** `Cut`/`MusicClip`/`Item`
`group`, `Project::groups`, `ProjectOp::Group`/`Ungroup`/`GroupLook`,
`describe`; `player/groups.rs` (Shift+click, Ctrl+G, Ctrl+Shift+G, a
click selects the group, Alt+click one clip); frames and bands on the
timeline; a dragged sound clip or item moves the group's others; Del
takes all. Not yet: merging a clip's pieces back into one, effects set
on all of a group at once, naming or recolouring a group in the window
(the op is there).

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
marks, and beats (found for gap 13, used by 57).

### 13. Cuts on the beat — medium (Kos TODO, 2026-10-23)

The TODO: "a cut doesn't land on the beat", found by ear. Find the music's
beats (onset or beat tracking: ffmpeg has no beat tracker, so a small Rust
onset detector, or a tool run as a CLI process), list the cuts more than
N ms off a beat (`omnix check` gains a `beat` rule), and draw the beats
as marks on the music lane that cuts snap to (markers, gap 5, and
snapping are there). Later (user, 2026-11-09).

**Agreed with the user, 2026-11-10** (what helps now: finding the beats
in a music clip, with a threshold of his own; cutting stays by hand,
with C):
1. The analyser finds a song's beats and draws them as purple vertical
   lines half the music lane's height; a threshold the user sets (lower:
   every beat, higher: the strong hits only).
2. A switch on the music lane: edges and the playhead snap to them.
3. Later, not critical: the user's own strong beats, full-height lines
   placed by hand on the music.
4. Explained, not asked for yet: a `beat` rule in the mix check (joins
   under music more than ~80 ms off the nearest beat, marked on the
   ruler) — for checking a cut made by ear.
The cutting view is gap 57 (`BEAT-CUT.md`).

**Built 2026-11-11, waits for the user's check** (1 and 2): `beats.rs`,
`omnix beats`, `Music::beats` and `SoundOp::Beats`, the purple lines,
the music lane's menu (shown, threshold, snapping), the beats in
`snap_targets`. 3 and 4 stay for later.

### 16. Subtitles from speech — medium (Kos TODO, 2026-10-23)

Mar'yana can't be heard even cleaned (action-cam mic, distance). Run
Whisper only on the cuts where someone speaks (the TODO warns CPU Whisper is
slow), on the cleaned voice. Put the draft as items on the text lane, one
per phrase, timed, to be fixed by hand. Recognition belongs in Dailix
(gap 83 finds the talk, 77 cleans the voice; transcription is gap 29).
Omnix gets the result as `text` lane items (existing format) or imports
an `.srt` (new). The same transcript later feeds text-based editing (9).

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

Where it stands (2026-11-09): built and checked, all but stingers
(postponed) and the 360 looks (wait for footage). The user wanted few
banal transitions: a library of non-banal ones, from gl-transitions
(MIT; each port keeps its Author and License), `TRANSITIONS.md` «The
library to build from». Every look exists twice, `transition::colour`
on the CPU (the export's `Dissolve`) and `transition.wgsl` (the
player's transition pass over two offscreen pictures, or one for an
edge), with a GPU test against the CPU at defaults and at every knob's
far end. Noise is the integer `transition::hash`, never a sine's.

Decisions that still hold:
- A join's overlap is keyed by the cut after it (`Cut::transition`,
  `transition_params`); an edge look belongs to its own cut
  (`edge_in`, `edge_out`) and goes where the cut goes. No ids of their
  own so far: the lane draws its zones from the cuts.
- Only what the user moved is stored; another look starts at its
  defaults; a split gives the join's look to the first half only.
- A fade to black takes the cut's sound with it (`Cut::sound_fades`).
- Templates are one library for the program, its folder made by Omnix
  at start (the user, 2026-11-08).

Steps:
- **25a. The overlap library and quick placement** (checked
  2026-11-08): whip pan, zoom punch, spin blur, flash, light leak, luma
  fade, glitch (`transition::Look`, `CutOp::Transition`); the
  Transitions panel (T, or a crossfade's X: `Selection::Join`): the join
  near the playhead and following it, Previous / Next, length by 0.25 s,
  tabs for the kinds, On every join, Remove, «▶ Play the join» (15
  frames before to 10 after, back to the start). Between two selected
  cuts waits for multi-select (gap 8).
- **25b. Edge transitions** (checked 2026-11-08): an out on one cut and
  an in on the next, no overlap (`Edge`, `CutOp::Edge`/`EdgeKnob`):
  fade (black or white), blur, zoom, glitch, flash; never drawn through
  a join's overlap; the Edges tab; an orange wedge on the cut.
- **25c. Templates** (checked 2026-11-09): a join whole (overlap, out,
  in: `template::Template`) saved by name to `<config>/transitions/`
  beside the LUTs and put on any join by `CutOp::Template`, all or
  nothing; the panel's Templates section (put on, save, remove asked).
- **25d. Stingers («перебивки», video transitions)** — postponed (user,
  2026-11-09: "more an extra than a must"). Both modes wanted: inserted
  between the cuts (the edit grows) and over the join (cinematic clips
  such as film burn or light flashes, blended). A short clip with its own
  sound, its sound on the SFX lane and linked to it; from the project's
  own clips. The over mode needs video on the overlay lane (gap 4, the
  B-roll milestone).
- **25e. 360 looks:** a whip round the sphere, little-planet zoom,
  sphere roll; wait for 360 footage (the camera not before ~2027-02).
- **25f. The transitions lane** (checked 2026-11-09; user, 2026-11-01:
  "our own collapsible field somewhere above the effects"): under the
  clip sound, over Effects, folding (kept in the settings). Its zones
  (`Timeline::zones`) show each overlap and edge look by name; a
  rectangle drawn across a join's middle makes an overlap as long as
  it, on one side an edge look on the nearer end of that cut
  (`Project::drawn_zone`), with the look chosen last; a click opens the
  zone in the panel; its length's edge drags. Later, with stingers: a
  stinger zone yellow.
- **25g. A look's own settings** (checked 2026-11-08): choosing a look
  opens a narrow panel beside the Transitions panel with its sliders and
  choices (`Look::knobs`; stored as `"transition_params": {"turns": 2}`,
  `CutOp::TransitionKnob`), a drag one undo step, Reset, copied by On
  every join; edge looks have theirs the same way.

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

Planned in Dailix, after gap 83 finds the talk (footage-agent's planned
`speech.py` was never built). faster-whisper locally on the RTX
3060 (large-v3 fits 12 GB; Ukrainian is supported), word timestamps, run on
the cleaned voice where one is chosen, else the clip's sound, and only
where there is speech. Written to the clip's sidecar. It feeds subtitles
(16), text-based editing, search by what was said, the bin and Info panel,
and later AI auto-edit cutting on phrases (28). Licence: faster-whisper
(MIT), Whisper weights (MIT).

### 30. Who is in the frame — medium (user, 2026-10-23)

Planned in Dailix (footage-agent's planned `faces.py`, insightface +
onnxruntime-gpu, was never built). The plan: take the named people's
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

### 34. Import from devices in the studio — medium (user, 2026-10-28)

Taken into Dailix (gap 74): ingest designed and decided in
`dailix/docs/INGEST.md`, built later. What follows is what footage-agent
had when the idea came.

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

### 37. Object tracking — large (user, 2026-11-01); part of gap 53 (53b) since 2026-11-02

**What the user means by it** (2026-11-10, to keep it straight): «an
animation tied to an object in the frame» is motion that follows
someone or something — a smiley over a person, subtitles under a
particular speaker, a label naming a thing — not the in/out presets
(87).

**The contract, built 2026-11-10** (`778c86c`, the user: "a contract for
objects' motion, for future tracking and tying objects to it"): a
tracker's output is `automation::ObjectTrack` on its cut (`Cut::tracks`:
a name, samples of clip time with the object's centre across and down the
frame as shown, 0..1 as an overlay's place, its size and turn when known,
a confidence), written by `CutOp::Track` (the tracker, or an agent).
Tying an overlay or text to it is `AutoOp::Follow`: its place values
(x, y, size, rotation) get automation clips from the samples where both
run, an offset added, marked `follows` (cut, track, offset) so they can
be made again when the cut moves. So a tracker only has to write tracks;
playing and exporting what follows them is automation already. The
existing `omnix track` (a coloured subject on a 360 clip, for the view's
follow) is the other half of this gap, the view's.

User: "object tracking" (recalled with the speed ideas; no detail yet).

What exists: `omnix track` and the colour-track picker follow one
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

**Which lane a tracked thing goes on** (user, 2026-11-04, asked; the
answer agreed by default): by what it is, not by whether it's tracked.
Pictures, emoji, icons, arrows and the cinematic bars (gap 15) on the
overlay lane; any words (subtitles, titles, names, labels, a place's
name following a person) on the text lane, which is drawn on top, so
words are never covered by a picture, and subtitles (gap 16) land there
already. Both lanes' items already have motion keys, so a track only
writes those keys (position, and size as the subject nears); the same
tracking serves either lane.

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

**Decided 2026-11-09 (user).** An edit in the Effects panel no longer
keys: a slider sets its effect for the whole cut or item
(`KeyOp::EffectValue`, `d8ee4ec`); no writing values on the fly.
Automation moves onto the Effects row, folding open into lanes, with
**automation clips as in FL Studio**: a block on a parameter's lane over
a stretch, holding a curve; outside it the slider's value holds. A clip
is **tied to its cut or item** (moves and trims with it, as keys do);
one across several cuts waits for groups (4B). Text and overlays have
effect rows of their own: the same there. Old keys are read into clips.
In the milestone «Ефекти, колір і розумний звук», after the Effects
panel's layout is sorted (the user's next ask).

**Built and checked 2026-11-10** (`60d1189` core, `9491af2`
cut lanes, `4203205` values on points and in the sliders, marks on the
folded row, `29ce7cd` text and overlay lanes, `9557bd6` an item's place
under its own Motion row, folding): `automation.rs`
(`AutoClip`: the value's name, from..to in clip or own time, eased
points, `follows` if made from a track); `graded`/`placed` over the set
values, read through `Cut::grade_at` and `overlay::look`; old effect and
motion keys read into clips spanning their cut or item; `Op::Auto` (add
flat at the value there, move, trim, delete, point set/move, delete,
ease, follow), no overlap of one value's clips (`check`), in the
property run and `describe`. The lanes are one piece (`timeline/auto.rs`,
`Lanes` over owners, a cut or an item, and values, an effect or a place
value) under the cuts' Effects row and each overlay and text lane's own;
the ◆ beside a slider in the Effects panel (effects and place) adds one;
each Effects row folds them and, folded, marks the clips and points. On a
lane: drag over empty lane draws a clip, a click in a clip adds a point,
drag a point, a clip or its edge, a point's value tag up or down (Shift
finer), Ctrl+click deletes a point, a right-click turns a point's ease or
takes a clip away. The panel's sliders show what plays at the playhead
and set the clip's point where a clip covers it. Next: the View row
(framing keys) by the same lanes; a clip made from a track made again
when its cut moves.

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
  neighbours, remove: `model::curve` (`Key`, `key_at`, the effect and
  motion keys) and the volume points made one.
- The timeline's lane drawing and mouse handling take a `Param` + `Curve`
  (timeline lanes behind one lane type).
- Users of it: Effects (11 parameters), View (yaw, pitch, roll, FOV),
  a cut's volume and the tracks' volume (already curves), speed (gap
  31), overlay motion and opacity, and a transition's knobs
  (`Look::knobs`), later a stinger's.

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
- What keeps the door open now: the core of ops (`model::ops`,
  `OPS-API.md`) and no new Linux-only code without a stub.

### 41. Core, functions and API: what to unify and separate — analysis (user, 2026-11-01)

User: "run a check and put into a separate file the possibilities of
unifying and separating out the core / functionality / API, which will
make further building simpler for us and divide the architecture
nicely." The second pass after gap 35's refactor, now that sound clips,
recording and the shared sound model (`project::Own`) are in. The result
goes to `CORE-API.md`; the steps it proposes become commits here. Analysis
done 2026-11-01 (`95a6be7`): ten steps, tests first; it also found the
contact sheet leaving a sound clip's own level out.

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
(from the attitude) or a colour track (`omnix track`, `src/track.rs`);
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
- **53c Angles as a multicam** (gap 49; now under «Мультикамерність», gap 73e): one 360 clip, several
  virtual cameras, cut between on the timeline; an angle per detected
  person later (53a).
- **53d Framing aids** to be taken from the research: rule-of-thirds and
  safe-area guides for the aspect, a horizon line, a «look room» hint,
  smoothing a hand-keyed path.

### 57. Cutting footage to music's beat, for the AI edit — idea, large (user, 2026-11-03)

Builds on the beats found for gap 13 (cuts on the music's beat, in the
user's editing order, 2026-11-07); this gap adds the ways of cutting.

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

The user's picture of it (2026-11-10), its own context: `BEAT-CUT.md`.

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

**Analysed 2026-11-04** (`GPU-PLAYER.md`, the user's ask): the decoding
is not the load. Playing and dragging the playhead, the player's main
thread (the interface) runs at 100 % of a core and ffmpeg at 5-15 %:
every timeline canvas is redrawn from nothing on every frame, tessellated
on the CPU, with no cache. CUDA already decodes (Vulkan crashes in this
ffmpeg); for 4K and 2.7K the CPU scaling after the download is most of
ffmpeg's cost, and `scale_cuda` before it cuts that 4-5×. For jumps a
fresh GPU decoder starts in ~0.45 s, so the scrub copy in software (0.05
s) stays. Order: measure the main thread's canvases, cache the timeline's
still layers, scale on the GPU, then the Settings choice.

**Measured and the first fix done 2026-11-04** (`8a2177d`), confirmed
by the user ("fire, mega smooth"; a short stutter, ~20 fps, only while a
sound lane's level changes and its waveform is drawn anew): the lanes' canvas took 40 ms a frame (79 fullscreen),
nearly all the waveforms; the sound lanes are now drawn once and kept
(`canvas::Cache`, keyed by a hash of what they show). The main thread
went from 100 % to 10-30 % while playing and dragging. Then (`b81a37b`, `a41e81e`,
confirmed by the user on the Kos clips): with CUDA the frame is
scaled on the GPU before it comes down (`scale_cuda`), 63-73 % of a core
-> 14-15 % for 4K and 2.7K in the bench. Left: the Settings choice. Not now (user, 2026-11-04,
"not critical"): the waveforms drawn as pictures on worker threads. It
would end the stutter at a level change, but the new waveform would then
show a moment late; seeing it at once is worth more while mixing.

### 67. Bokeh: the background softened behind a person — idea (user, 2026-11-04)

The user asked what the plan holds for the picture («bokeh and so on»):
only a whole-frame blur (gap 26). A real bokeh softens what's behind the
subject and keeps the subject sharp, so it needs the subject cut out:
without a depth camera, a segmentation model (a person mask per frame,
run once per clip like the colour track, its mask cached), then a blur
weighted by the mask, keyframed like an effect, CPU and GPU alike with a
parity test. On a 360 view the mask comes from the framed picture.
Belongs with face detection and tracking in milestone «Кадрування» (gap
53); the model's licence is to be checked (no AGPL, as with YOLO).

### 68. Curves and colour by hue — idea (user, 2026-11-04)

Today's Effects work on the whole picture. Missing: tone curves (RGB and
per channel) and adjusting one hue only (HSL: the grass greener, the sky
deeper, the skin left alone), as Resolve's and Premiere's colour tools
do. Kept in the effect keyframes like the other values (new optional
fields), mirrored in the shader and `Grade::apply` with a GPU-CPU test.
Milestone «Ефекти, колір і розумний звук», after the black and white levels (gap 14).

### 71. Two viewers: the source and the edit; effects tried beside the original, then applied — idea (user, 2026-11-04)

User, on the skimmer card (gaps 2, 9): "I don't know if it'll be needed,
whether it's worth anything. Wouldn't it be better here to add a double
viewer: from the timeline and from a specific chosen file. And, as in
Kdenlive, apply effects on a separate screen at the side, to compare with
the original. A toggle: off, editing writes straight into the timeline;
with the second screen open, effects are added only when you press
Apply, and their values move as separate line-rectangles into the
«effects» group, with curves of the values."

**71a. Two viewers** (source monitor and program monitor, as in Kdenlive,
Premiere, Resolve). A file chosen in the bin plays in a viewer of its own
beside the edit's, with its own in and out points; the add buttons put
the marked span on the timeline. This is the «Перегляд з біна у вікні
Omnix» card made concrete (today a clip previews in ffplay's own window).
In and out points (user, 2026-11-04): several segments per file, the
same as the contact sheet's marks (`{t, end, role, note}`), which Omnix
reads in and shows on the bin player's strip; a segment picked there is
what the add buttons add; I / O set a new one. Both ways with the sheet
is gap 72e (`footage-agent's folder/docs/CONTACT-SHEET-NEXT.md`, section E).
The skimmer keeps a smaller job: on the timeline, a look at a frame
without moving the playhead; in the bin the source viewer replaces it.
Milestone «Матеріал і бін».

**71b. Effects tried beside the original.** A toggle in the Effects
panel: off (the default), changes write into the edit at once, as today.
On, the second viewer shows the cut before and the first after, and
nothing reaches the edit until Apply; then each effect's values land on
the effects group (gap 39's automation lanes) as a block with its value
curve. Needs gap 39's lanes for «as separate lines». Milestone «Ефекти, колір і
розумний звук». Note: the effects split line (`SplitDrag`) already compares
before and after inside one viewer; 71b adds the side by side and the
staging.

### 72. The contact sheet's next round — ideas (user, 2026-11-04)

Kept in a context of its own in footage-agent:
`footage-agent's folder/docs/CONTACT-SHEET-NEXT.md`. In short:
- 72a: tie a mark, clip or stretch to the song meant for it, as a hint
  for the AI edit (to a point in the song too: "on the drop");
- 72b: place a ready song under clips in the sheet, exported by «→
  Omnix» onto the Music lane, Omnix's ducking doing the rest;
- 72c: the sheet's look and working flow, to think through together;
- 72e: the sheet's in/out segments in Omnix: shown on the bin player's
  strip, used by its add buttons, set with I / O; written back to the
  sheet (decided 2026-11-04). Built the same day (`notes.rs`,
  `bin_player.rs`; footage-agent refuses a stale save), waiting for the
  user's check;
- 72d: what an AI editor wants in `notes.json` (a brainstorm there:
  spans, story order, ids for marks, closed decision words, music as
  data, a schema, a compact text view).

Milestones: 72a, 72d «ШІ-монтаж» (they feed gap 28's brief); 72b, 72c
«Матеріал і бін».

### 73. Multicam as a module: several cameras as one clip — idea, large (user, 2026-11-04)

User: "multicam editing (POV video of one ride from several cameras) as a
module, so the linked videos become one pseudo-file on the edit's line.
A module of its own, like reframing: it opens inside, in the screen's
place, or everything folds but the video row, for more room. 360 video
is a module of its own too (pseudo-multicam there). The two can be
turned on separately. The multicam pseudo-file can be put in the bin, a
kind of file to add to the edit. Why a «kind of file»: so as not to
breed copies of the video files."

What it is in the trade: Final Cut's multicam clip, Premiere's multicam
source sequence, Resolve's multicam clip.

- **73a The multicam clip** (the «pseudo-file»): a small file in the
  project's folder (e.g. `01_footage/<name>.multicam.json`) naming its
  angles: each a real file, read in place, and its offset on a shared
  clock. Synced by the cameras' recorded time, then by the sound (each
  camera's audio matched against the others, as PluralEyes does), then
  by hand. No video is copied or re-encoded.
- **73b In the bin**: listed as a clip of its own kind (a stack of the
  angles' pictures), added with the same buttons; Info lists the angles.
- **73c On the edit**: a cut of a multicam clip names it plus the angle
  shown, and a switch of angle is a cut point (so trimming, J/L and
  everything else work as for any cut). Optional fields only.
- **73d The module**: opens in the picture's place (as the bin view and
  the mix check do), or folds everything but the video row: the angles in
  a grid, the edit playing; a click or a number key switches the angle
  at the playhead, live while playing. Sound: one chosen camera's, or
  each angle's own.
- **73e 360 as its own module** (gap 49 / 53c): the same idea with
  virtual cameras from one sphere; switched on separately from 73d.

Milestone «Мультикамерність» (new, 2026-11-04, after «Кадрування»: 73e
builds on its angles). Needs the core API for the new cut kind.

### 74. Dailix: the studio and contact sheet in Rust, a project of their own — idea, large (user, 2026-11-04)

The user wants footage-agent's web contact sheet and Python server
replaced by a native Rust app akin to Omnix (one set of tools; maybe
published some day), separate from Omnix because their needs differ:
media from cards and phones into a staging place (TMP), chosen in the
contact sheet, then moved into a categorised archive (disks, buckets;
one copy now, two later) and an Omnix project. Repo
`../dailix`, context `docs/CONTEXT.md` there (what moves
over from Python and how, the open decisions). Shares data with Omnix
(`notes.json`, sidecars, the folder tree), later a small common crate.
Takes in gap 34 (import from devices) and 61 (the sheet standalone).
Milestone «Матеріал і бін». Built so far: the contact sheet's core
(2026-11-04, `dailix/docs/SHEET-CORE.md`; Omnix's bin runs on it,
`98f2795`), its window (culling, marking keys, Omnix's player beside),
Home of trips with their videos and days (gap 82), Settings, a new
trip; Kos is its first project. Still to do, by its own context: the
parity steps (voice clean-up 77 among them) before the web sheet
retires, then ingest (34, `dailix/docs/INGEST.md`).

### 78. A vertical video in a wide edit: its sides filled from itself, not black — idea (user, 2026-11-05)

Phone clips are often vertical; in a 16:9 edit they stand between two
wide black bars. The usual answer: the same clip, scaled to fill the
frame, blurred and darkened, behind the sharp one (pillarbox fill),
or a softer kind: the edge colours stretched outwards. It belongs with
the overlays (the clip over a background of itself): a cut's option,
on by default for a clip whose shape isn't the edit's, the same in the
player (GPU) and in the export (ffmpeg `split`, `scale`, `boxblur`,
`overlay`), tested to match. Milestone «Кадрування».

### 84. A video drawing on several trips («a year in review») — idea, backlog (user, 2026-11-07)

A trip's videos take only its own footage (gap 82). A video made of
several trips would name the other trips' clips (by trip and clip key),
not copy them; the bin would show those trips' days too. Nothing in gap
82's layout stands in its way.
Milestone «Матеріал і бін».

### 85. Project cards in a grid with a GIF on hover — idea, backlog (user, 2026-11-07)

Omnix's start window (recent edits, «Browse projects») and the trips'
lists show plain rows today; they do their job. Later: cards in a grid,
each with basic facts and a short description, and a moving preview of
the project (a GIF-like loop of a few frames) played only while the
pointer is over the card. Dailix's trip cards (gap 82) are the first
step; the hover preview could reuse the bin's hover frames (gap 60).
Milestone «Матеріал і бін».

### 86. Animated cutaways from a prompt — a program of its own, later (user, 2026-11-07)

User: a separate, fully open-source program: from a prompt and
parameters it makes an animation for a cutaway. Example: a voice-over
says "we took the scooter, rode a mountain path, reached the castle, but
there were 300 steps... so we turned round and went to the nearest
tavern for ice cream", and the animation tells it in the style of an
8-bit game, or of slides with animations. Not instead of Omnix and
Dailix: after the editing side is as complete as it gets, once the user
edits the clips marked in the contact sheet.
- First: look at what exists and its licence (code-driven animation
  tools such as Manim or Motion Canvas; whether any takes a story and a
  style), so it builds on an open one rather than from nothing.
- The input (user, 2026-11-07): not the voice-over, a much simpler
  parametrised description (scenes, who, what happens, the style); free
  models only, nothing paid; free outside services acceptable. Asked
  first for the real possibilities: procedural animation from sprite and
  slide templates, local image models for the assets, local
  text-to-video for short moods (answered in the session; to research
  properly before building).
- Its output: a video file (with alpha where it helps) that lands in a
  trip's `06_external` and on Omnix's overlay or main lane as a stinger
  (gap 25d).
- Non-commercial (user, 2026-11-07: «what Linux lacks»), so models free
  for non-commercial use are open to the user himself; the program, if
  published, is better built on freely licensed ones. Ready Linux tools
  to try first: ComfyUI (local, generation of pictures and short videos
  through workflows: Wan, LTX, AnimateDiff), Glaxnimate (vector
  animation, inside Kdenlive), Synfig, Blender's Grease Pencil, Manim and
  Motion Canvas (code), Pixelorama / LibreSprite (pixel art). None known
  to take a scene description to an 8-bit cartoon end to end, offline.
- Next (user, 2026-11-07): the user tries ComfyUI first; if it suits,
  this program isn't needed. Ask how it went before any work here.
Milestone «Ритм і переходи».

### 88. The Osmo 360 II — when the camera comes (numbered 2026-11-09)

The card «Osmo 360 II» (milestone «Якість 360») had no number. The user's
360 camera will be an Osmo 360 or Osmo 360 II, not before ~2027-02. With
it: its lens calibration read as the Osmo 360's is, and decoding 8K50
fast enough (about twice the Osmo 360's frames; `CONTEXT.md` «Machine»
measured ~2× headroom on the 3060, unverified). A raw Osmo 360 II sample
exists (found 2026-11-09, `CONTEXT.md` «Samples»): its calibration and
decoding can be tried on it before the camera comes.

### 95. Text: its own colour, letter spacing, a box behind it — idea (Claude's UI review, 2026-11-10)

From gap 94's review, for later: the text's colour is one of a few
swatches (no colour of its own); no letter or line spacing; no box
behind it (a caption's plate), common in editors. Its colour could be
automatable as the effects are. Milestone «Ефекти, колір і розумний
звук».

### 96. A timeline that scrolls up and down — idea (user, 2026-11-10)

The user: with many lanes of clips and automation the timeline outgrows
its place. A switch on the panel, «Scrollable»: while on, the lanes keep
the size they had when it was switched on, however the window grows,
and a scroller appears for up and down (Alt + the wheel too). Off, the
lanes fit the place as today.

- Today the rows scale with the timeline's height (`Rows::new`'s
  scale); the switch freezes that scale and lets the content run taller
  than the place.
- The header column scrolls with the lanes; the ruler stays on top.
- Alt + wheel scrolls; the wheel alone keeps what it does now.
- Milestone «Зручний таймлайн».

### 99. Sync lock on a sound track — medium (user, 2026-11-11)

The user cut a film to a song's beats (gap 13), then trimmed a few
seconds off the first cut's start: the edit is magnetic, so every cut
after it moved left, while the song stayed where it was — every cut
fell off its beat.

Agreed (2026-11-11, Claude's plan, the user approved): **sync lock**,
Premiere's name. A lock on a sound track's head; while it's on, an edit
that makes the picture shorter or longer at some point (a cut trimmed,
deleted, added, a crossfade changed) moves that track's clips after the
point by the same amount. A clip across the point moves too and loses
(or gets back) as much at its head, so what's after the point stays
under the same pictures.

- A rule of the core (`Project::ripple(at, by)` or so), run inside the
  cut ops when a track is locked; a setting of the track (`Music::sync`)
  and an op; `describe` says it. Tests: trimming the first cut's start
  keeps a beat under the second cut's start.
- Weighed: FCP's connected clips (a song pinned to one cut: a song under
  many cuts pins badly) and groups (gap 8: they need the same shift to
  keep a song with its cuts). Sync lock first; groups use it later.
- Milestone «Ритм і переходи».

**Built 2026-11-11, waits for the user's check:** `Music::sync`,
`SoundOp::SyncLock`, `ripple_of` and `Project::ripple_locked` run after
every cut op; «Sync lock» on a sound lane's menu, «· sync» on its name;
`describe` says it. A clip starting right at the point counts as across
it (a trim dragged there and back puts it back). Beats found to the
sample on the way (gap 13).

### 101. Nothing longer than the film — medium (user, 2026-11-11)

The user, grouping clips: music, text, an overlay — anything beside the
pictures — must never run past the film's end (its length at most the
film's), so no pieces of music, effects or overlays are left hanging
over other videos. Planned (step 4 of the plan in CONTEXT): they stop
at the film's end when dragged or trimmed (red, a hint), the ops hold
them there, `apply` and the mix check warn of what's past it, «Trim to
the film's end» on a lane's menu. Milestone «Зручний таймлайн».

**Built 2026-11-11, waits for a check.** Decided on the way: a project
with no pictures has no end (a song laid down first runs free); a
thing already past the end (longer than the film, or the film cut
shorter) may move earlier or get shorter, never later or longer — else
a long song would jump to 0 on any drag; a dragged group stops whole.
Adding a clip doesn't trim it: the warning, the red mark and the trim
say it instead. `TrimToEnd` takes away what starts at the end or later.

### 100. Outside material in Dailix: by link and dragged in — medium (recorded 2026-11-04, «Далі» 2026-11-11)

From footage-agent's `grab.py`, recorded in Dailix's CONTEXT: yt-dlp
run as a CLI program (never linked); a URL, an optional span and a
name; music into `02_audio/music`, sound effects into `07_sfx`, video
into `06_external`; a line in that folder's `LICENSES.md` (where from,
the licence, the date). In the window: paste a link, pick the kind;
`dailix fetch` by command. Omnix's bin shows them in its sections.
The user moved it to «Далі» and grew it (2026-11-11): pictures too, dragged
into the window or by link, converted to what the project keeps (JPEG,
WebP and the like → PNG); a milestone of its own, «Додаткові матеріали».

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
   → 9 skimmer on the timeline (needs 10 for S; in the bin 71a replaces it) → 9 action bar → 9 typed time and UI scale
   → 27 overview strip, centred playhead option.
3. **Матеріал і бін** (material in): trips holding their videos in
   Dailix (82), «New project» through Dailix (80 → 82), both done; ratings
   from Dailix, LUTs, E/W in the bin (2); ingest in Dailix (74 with 34;
   one's own videos first); the source viewer beside the edit (71a)
   later (user, 2026-11-09: re-cutting is done in Dailix for now).
4. **Додаткові матеріали** (user, 2026-11-11: filling a project with
   outside material): 100 — music, sound effects and video by link
   (yt-dlp), pictures dragged in or by link, converted to the format the
   project keeps (JPEG, WebP → PNG and so on), each with its licence in
   `LICENSES.md`.
5. **Ритм і переходи**: 25a basic transitions with quick placement →
   25g a look's settings → 25b edge transitions → 25c templates → 25f
   the lane (all built 2026-11-08) → 25d stingers → 25e 360 looks (with
   footage); 13 beats later (the user, 2026-11-09: re-cutting can be done
   in Dailix, not the most needed now).
6. **Ефекти, колір і розумний звук** (user, 2026-11-09: next after
   transitions; "effects, colour, animation, texts, pictures, object
   tracking, understanding speech, better sound through Dailix's analysis
   and suggestions from different models, so text/music auto-ducking
   works not only on the voice-over but on the original videos"): what
   was «Колір і текст» (69 scopes → 14 levels → 68 curves and hue → 71b
   effects beside the original (after 39) → 15 bars → 26 glow/blur → 6
   vignette shapes → 16 subtitles → text-based editing; 70 text and
   overlay groups; 43 the palette), 87 style looks and animation presets
   for text and pictures, 37 object tracking, 83 where people talk with
   77's clean voice and level, 81 auto-duck under talk in a cut's own
   sound, 29 speech transcription. Order agreed with the user 2026-11-09:
   style looks (87 with 26 glow and blur, 6 vignette shapes, 15 bars) →
   text and picture animations and overlay kinds (87, 70) → colour tools
   (69 scopes → 14 levels → 68 curves, 71b) → speech and sound in Dailix
   (83 + 77 → 81 → 29 → 16 subtitles, text-based editing) → tracking (37).
7. **Розуміння матеріалу**: 30 who is in the frame (names from Immich);
   speech (83, 29) moved into 5.
8. **B-roll і групи**: video on the overlay lane (cheap cutaway) → 4B
   group/ungroup + connected clips + 8 multi-select, one format change,
   designed as one.
9. **Кадрування** (gap 53, user 2026-11-02): framing grown up from gap
   10's switch: research the best practices first (Insta360 Studio, DJI
   Studio, GoPro, Auto/Smart Reframe), then faces and people (53a, with
   Dailix's 30), framing aids; object tracking (37) moved into 5, angles
   as a multicam (49, now 73e) a card of its own.
10. **Мультикамерність** (gap 73, user 2026-11-04): 73a the multicam
   clip (a file naming its angles, synced by time, then sound) → 73b in
   the bin → 73c cuts of it → 73d the module → 73e 360 angles the same
   way (gap 49 / 53c).
11. **Швидкість і автоматизація** (gap 31): constant speed per cut with
   its sound following (atempo), measured for drift → speed ramps →
   automation lanes per value (the user's FL Studio idea). Large and
   careful; after B-roll and groups, since it touches the same time
   mapping. With it: 31b keep-the-pitch switch, 31e speed points on the
   main timeline, 31c the speed editor window, 31d right-click keyframes,
   and the automation idea that 37 object tracking (in milestone 5) keys
   into.
12. **Якість 360** (from `OPENOSV.md`, mostly needs 360 footage: one public
   sample now, the user's camera ~2027-02): 17 easing presets and 18 bundled
   LUT any time (small, useful now) → 24 file facts → 19 lens protectors →
   20 mask release → 21 tilt diagnostic → 22 sky seam field (patents first)
   → 23 DJI presets → Osmo 360 II.
13. **Експорт** (gaps 51, 52; user, 2026-11-02): what the export makes
   (4K and 8K, HEVC and AV1, bitrates, frame rates, sound, presets, HDR
   and 360 out) and how fast (the frames' path through the GPU). Before
   the AI edit, which needs it; reordered freely.
14. **ШІ-монтаж** (gap 28, `AUTO-EDIT.md`): the user's main wish, but put
   last on purpose (user, 2026-10-23: "I wouldn't do AI functions yet:
   Omnix doesn't have its full working functionality"). The design stays
   ready. The pieces it needs arrive with the milestones above.
15. **Інші платформи** (gap 40, user's question 2026-11-01): Mac and
   Windows realistic (upstream builds both), Android a new UI over the
   same core. Not planned; no new Linux-only code without a stub.
