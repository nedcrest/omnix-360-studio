# Editor gaps — done

The finished gaps of `EDITOR-GAPS.md`, moved here on 2026-11-04 so the
open ones read quickly; same numbers. Each says what was built, how it
was checked, and what was left for later (if anything is, it is a card
of its own now). New finished gaps move here when their last card in
`ROADMAP.md` reaches «Зроблено».

## Where it started (as of 2026-10-22, historical)

The timeline as the first real edit found it. Kept for the record: much
of it is no longer true (sound lanes, markers, inserting at the
playhead, the bin all came since).


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


## Done gaps

### 1. Insert a clip between two cuts — small

User: no way to "split the video and put another clip in there".
Today it takes three steps: C at the point, add the clip (lands at the end),
drag it back. Fix: an **insert at the playhead**: if the playhead is inside a
cut, split it there and put the new cut between the halves; on a boundary,
just put it there. Used by the bin (item 2) and by Ctrl+O. No format change.

**Done** (noted 2026-11-04): insert at the playhead (`Place::At`: inside a cut it splits it, on a join it goes there), used by the bin's buttons, +Blank (32) and the ops API.

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
menu, and `omnix duck project.json [--depth] [-o]`. On the test
project: speech 5.7–11.7 s and 12.9–25.9 s, the music 12 dB down there.

**Not yet:** two sound effects at once (a second SFX lane).

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

### 7. A wider selection frame — small (user, 2026-10-22)

The selected cut's frame on the timeline is hard to see. Make it thicker
and in the accent colour (`timeline.rs`, where the cut blocks are drawn).

**Built 2026-11-02 with timeline pack 1** (`b5c95f8`), confirmed by the
user the same day: 3 px in the accent colour over a dark 5 px line, on cuts,
sound clips and overlay items (`selection_frame`).

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
  `omnix normalize`, the export limiter (0.87 = −1.2 dBFS, `level=0`,
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

**Done** (noted 2026-11-04): the export's mix ends in `ffmpeg::mix::LIMITER`, peaks held under −1.2 dBFS (−1 measured 0.2 dB over between samples).

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
- `omnix blank DIR` makes the same file and prints its path, so a
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

**Done** (noted 2026-11-04): the Clip sound lane's right-click menu has all of it, and grew with 33b–33i and 44.

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
export's), its clips measured, plus `omnix mixcheck`; (3) the view.
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


**Done 2026-11-04** (`32b75a9`), confirmed by the user on a wired keyboard: F1
opens and closes it; «?» never reached the app, so it was dropped.

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

### 60. The bin as a view of its own, in tiles — done 2026-11-04, confirmed by the user 2026-11-06 (user, 2026-11-03)

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

Built 2026-11-04 (`bin_view.rs`; `c059e1f`, `cbaaea6`, `07c03b5`, `c5808fd`): the
file picked shows at the right with its Info, the edge between dragged
(user's ask the same day); a tile's click only shows. A clip plays
there in the bin's own player (`bin_player.rs`, `e3dd968`): a second
`Playback`, a strip to scrub, Space; the first part of 71a (in and out
points still to come). The pictures stop at half the view's height. The Bin
toggle in the Mode group or Shift+B; every section shown, an empty one
too, with its «+»; a drop goes to its kind's first section (Footage,
Music, Graphics), since X11 drops carry no position; the tile says where
the edit uses the file; the trash is the system's (`trash` crate). Left
for later: ratings and the skimmer (gap 2), LUTs as a section, moving a
file between sections. Drops don't arrive on Wayland (winit).

It grows the narrow bin panel (gap 2) rather than replacing it: the panel
for quick adding while cutting, the view for sorting the material. Joins
the «Перегляд з біна у вікні Omnix» card: the view where a tile is
previewed is the same place.

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

### 66. The button groups in colours of their own — done 2026-11-04 (`5b85986`), confirmed by the user

User: "different colours for the groups of buttons for editing and for
navigation, so what's needed is easier to find by colour patterns in the
UI (now the buttons are all blue)". Builds on gap 59 (the bars grouped)
and gap 43's palette proposal (`UI-COLOUR.md`, «Omnix Neutral» and the
lanes' colours): a muted tint per group, e.g. adding at the playhead,
cutting, history, playing and moving about, framing, panels, each the
same wherever it appears (the shortcuts window's groups too), and never
colour alone (icons and the groups' names stay). Milestone «Колір і
текст» with the palette card.

**Built 2026-11-04** (`5b85986`): `icons::Tint` and `tint_button` (lighter
hovered, darker pressed, faint disabled). Global slate `#4A505A` (app,
project, export, window, the chevron), Move blue `#2F5F9E` (play, zoom,
snapping), Add green `#3A7A50`, Cut amber `#9A6026`, History grey
`#5E5E5E`, Look violet `#6E559E` (as the effect marks), Frame plum
`#8A4678` (the framing row), Sound teal `#26787E` (as the clip sound).
The shortcuts window shows each group's swatch. Panels' own buttons keep
the theme's.

### 75. A cut's flags from the contact sheet as markers on the edit — done 2026-11-05 with from-sheet, confirmed by the user (user, 2026-11-05)

In Dailix a flag (F) is a note to self, never material: «a meme here»,
«find a laughing sound». When a segment goes into an Omnix edit, the
flags inside it would come along as markers with their notes, at the
same place in the cut; so the timeline says where the meme or the sound
goes. The markers exist (gap 5, built 2026-11-03: `Cut::marks`, M,
the Markers row); this is gap 5's one leftover, «footage-agent's flags
exported into them», now from Dailix's sheet.
A flag lands in its cut's `marks`, in clip time, so it travels with the
cut (user, 2026-11-05: «move a video's flags with it»): moved with the
cut, kept on its frame through a trim (hidden past the cut's ends, back
when trimmed out again), carried by both halves of a split. Must stay
so; `markers_are_set_noted_moved_and_removed_by_time` checks it.
Built with `omnix from-sheet` (`src/from_sheet.rs`, `f0b37ed`): a Dailix
project's marking made into an edit by footage-agent's «→ Omnix» rules,
a clip's flags inside a cut its markers; Dailix's «→ Omnix» by each day
calls it. The bin also lists a section's subfolders (Dailix's days,
`f522ab7`). Flags outside every
segment stay in the sheet. Made when a cut is made from a segment (the
bin's add buttons, or Dailix's hand-over), kept in step by the mark's
id (a marker could carry it). Milestone «Матеріал і бін».

### 76. The program's name: rosmo360 → omnix — done 2026-11-05, confirmed by the user 2026-11-07 (user, 2026-11-05)

The binary is still upstream's `omnix`. To do later, as one change,
because the name is in 34 files across three repos:
- the Cargo package and binary (`Cargo.toml`), and with it the library's
  crate name Dailix uses (`rosmo360::viewer`, `rosmo360::from_sheet`);
- the settings and cache folders (`settings.rs`, `cache.rs`:
  `~/.config/omnix`, the clip cache): moved or read from the old
  place once, so nothing set or cached is lost;
- `packaging/` (desktop entry, icon names, Flatpak, macOS, Windows);
- the CLI's examples and docs; `tests/cli.rs`;
- footage-agent's `export/omnix.py` (`app()` looks for `omnix`) and
  Dailix's `omnix()` (OMNIX_BIN, PATH, the sibling build);
- the desktop entries installed on this machine.
Upstream (rOsmo360) is unchanged since our fork point; merging from it
later gets harder with the rename, so it waits until we know we won't.
Milestone «Інші платформи» (packaging).
Done 2026-11-05, a commit a step: the crate, library and binary (`omnix`;
Dailix's dependency and its module `editor.rs`); the settings and cache
folders (`~/.config/omnix`, `~/.cache/omnix`, the old ones renamed over on
first use, tested); `OMNIX_CACHE`, `OMNIX_10BIT`; packaging (with a
placeholder app id `local.omnix.Omnix` and no release repository until
published); the docs; footage-agent's lookup. Kept: the cache's file mark
`rOsmo360` (changing it would remake every entry) and the 0.7 project
fixture.

### 79. Omnix's own face: the footer's credit and the start picture — done 2026-11-05, confirmed by the user 2026-11-07 (user, 2026-11-05)

The footer (`player/icons.rs`) still reads «Omnix · built on rOsmo360 …
· Made by Vibing with Ravelund · 2026», and the picture shown while it
starts (`LOGO`, `vibewithravelund-lowres-corrected.jpg`) is upstream's
logo, which its README keeps out of the licence («all rights
reserved»): it can't stay in a program we might publish. User: "I write
this with you, with Claude: that belongs there."
- Footer: Omnix and its version, made by Dmytro Halushko with Claude,
  2026; upstream credited in a line of its own (built on rOsmo360 by
  Vibing with Ravelund, MIT/Apache), as the licence and fairness ask.
- Start picture and window icon (`packaging/rosmo360.png` is upstream's
  too): Omnix's own, drawn as an SVG (a sphere over a timeline, say), in
  the style of Dailix's `assets/dailix.svg`; the user picks from two or
  three drafts.
- README's credits and licence notes the same.
Done together with the rename (gap 76): name, icon, footer and start
picture are one change of face. Milestone «Інші платформи» (packaging).
Done 2026-11-05: the user's pick A2 (a sphere over a timeline, a playhead
through it), `packaging/omnix.svg` and the PNG/ICO made from it; the
footer; README's credits. Left for publishing: the screenshots
(`docs/screenshot.png`, `docs/guide/`) are upstream's, all rights
reserved, to be taken again in Omnix; the app id and release repository
are placeholders (`local.omnix.Omnix`, `REPO=` in `install.sh`).

### 80. Omnix starts at its recent and found projects; a new one is made in Dailix — done 2026-11-08, confirmed by the user (user, 2026-11-05..07)

For the release: Omnix opens at the projects opened last and those found
in the projects place; «New project» goes to Dailix, where a project is
made in the one fixed layout (`dailix_sheet::Layout`: `01_footage/<day>/`,
`04_project` for edits, ...). The point (user): "a fixed structure of
projects, so I never wonder where that video is". Both are standalone, so
each has its projects folder chosen in its window; then Omnix runs from a
desktop icon without opening Dailix first.

Decided (user, 2026-11-07): Omnix keeps a projects place of its own,
Dailix's until one is chosen in Omnix; the order below, own videos after.
1. The projects place in both windows — built: Omnix's Settings
   «Projects» (Choose…, Use Dailix's; `fsutil::dailix_projects` reads
   Dailix's settings file, whose place both take from
   `fsutil::dailix_settings`); Dailix's «projects» chip opens a folder
   dialog. Checked both ways (user, 2026-11-07), who then asked for:
1b. Settings as a window over the program, in groups, in both (built:
   Omnix General / Picture / Editing, also from the start screen;
   Dailix Places / Look, a Settings button and Ctrl+,, every place's
   chip choosing its folder).
2. Omnix's start window (user, 2026-11-07): the first run, the start
   page as now; every run after, the 5 edits opened last to open, and a
   browser to toggle open of Dailix's projects: each global project and
   the video projects inside it (gap 82), with their edits. Built
   without gap 82's inner folders (each project's `04_project` edits):
   `dailix_sheet::project::{all, edits, of_edit}`, `settings::remember`.
   Dailix too (user, 2026-11-07): Home always in its bar, the first run
   at home, every run after where it was closed (project and clip).
3. Omnix's desktop entry and icon: built (`omnix install`; one installer,
   `omnix::desktop`, for both programs).
4. «New project» in Omnix opens Dailix, which makes an empty project in
   the fixed layout: moved to gap 82, step 3 (it needs the trip's video
   folders). Steps 1-3 checked (user, 2026-11-07).
5. Later, its own card: adding one's own videos to a project in Dailix
   (copied to `01_footage/<day>/`, a sidecar and a poster each): ingest's
   first step (`dailix/docs/INGEST.md`).
Milestone «Матеріал і бін».

### 82. Dailix: global projects holding video projects; their home — done 2026-11-08, confirmed by the user (user, 2026-11-07)

User: the Dailix place holds global projects (a trip, an event, a hike, a
motorbike ride); inside one, a folder per video made from it, so several
videos from one trip each sit in a logical folder of their own.

Decided (user, 2026-11-07):
- The global project is the trip as today's Dailix project: all its
  footage by days, its marking. Its videos are folders inside it:
  `videos/<name>/` holding the video's edit (`edit.json`) and its
  `export/`; they all take the trip's footage.
- The projects place is the global Dailix folder (`DAILIX/`), for both
  programs; the test project moved there (`DAILIX/2026-08_test`, the
  `TESTS` place gone).
- Dailix's Home is a page of its own: a card per trip (its poster, name,
  dates, days and clips, a description and tags from `project.json`). A
  card opens the trip's page: its days (the cards of today's home) and
  its videos.
- Dailix opens where it was closed (Home, a trip, a day's clip).

Steps, each checked by the user:
1. Home: trip cards; a trip's page with its days and its edits (the
   loose `04_project` ones until 2); description and tags shown and
   edited on the trip's page. Built (Dailix `e2687c5`), checked
   (user, 2026-11-07).
2. Videos: built (Omnix `530ee81`, Dailix `a9a94f1`), checked (user,
   2026-11-08). Decided (user, 2026-11-08): a new video is named for its day or
   `all-days` (`-2` when taken); one `edit.json` per video (a variant
   later as «duplicate the video»); exports default to its `export/`;
   the old `04_project` tidied: one copy of each Omnix edit moved into a
   video, copies and empty ones to the trash, `04_project` and
   `05_export` gone from the layout (`dailix tidy-edits`, run on Kos and
   the test). The two Kdenlive working copies in footage-agent's old Kos
   folder went to the trash too (user's word, 2026-11-08).
3. «New project» (gap 80 step 4): built (Dailix `868e3bb`, `bf2452f`, `c0ab76e`;
   Omnix `2300550`, `eac86b4`, `aa0cd93`), checked. Decided (user,
   2026-11-08): a trip's folder is its month and its name in Latin
   (`2026-11_motopokatushky-lystopad`, editable); Omnix's «New…» offers a
   video in a trip, a new trip in Dailix (opens on its form), or an edit
   without a trip; adding one's own videos stays its own card, after the
   editing work. Dailix: «+ New trip» on Home, Ctrl+N, `dailix new-trip`.
   One way to find and start the other program: `omnix::sibling`.
Milestone «Матеріал і бін».
