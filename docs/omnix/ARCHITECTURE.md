# Omnix architecture — where things live, and where they are going

Written 2026-10-28, from a full read of the code by two analysis passes
(gap 35 in `EDITOR-GAPS.md`). The user asked for "an analysis of the
code, a refactor so it scales easily and is API-driven: not everything in
one script or one class, properly separated, and documented". This page
is the map and the plan. Rewrite it as the code changes: it describes the
code as it is, not its history.

## The shape today

~21 500 lines of Rust in one binary crate, `rosmo360`.

| Layer | Files | What it is |
|---|---|---|
| Stitching and colour (upstream) | `stitch.rs`, `stab.rs`, `view.rs`, `align.rs`, `exposure.rs`, `ring.rs`, `imu.rs`, `grade.rs`, `lut.rs`, `hdr.rs`, `dji.rs`, `track.rs`, `sky.rs` | The 360 maths: calibration, stabilisation, seam, framing, grading. CPU reference; the shaders mirror it. |
| Media I/O | `ffmpeg.rs` (1050), `cache.rs`, `loudness.rs` | ffmpeg as a separate program: probing, decode commands, the encoder, the export's audio mix graph; the clip cache; loudness measuring. |
| The project (the file format) | `project.rs`, `overlay.rs`, `text.rs` | `Project`, `Cut`, `Music`/`SoundTrack`, overlay `Lane`/`Item`. Public JSON, backward compatible. |
| CLI and export | `main.rs` (1790) | The clap commands, **and** the export pipeline (`Pipeline`, `cut_pipelines`, `export`), contact sheets, `scan`, `seam`, `info`, `track_to`, file helpers. |
| The player (GUI) | `player/mod.rs` (4400) and 25 sibling files | iced app: `App` holds all state, `Message` has ~150 variants, `handle()` is 1100 lines, `view()` 384. Some areas already live beside it as `impl App` blocks (`bin.rs`, `sound_ui.rs`, `effects.rs`, `angles.rs`, `info.rs`, the panels). |
| Edit model with undo | `player/edit.rs` | `Edit`: the project's fields copied one by one, with undo snapshots; tested split/trim/move/insert. |
| Timeline | `player/timeline.rs` (1360) | Two canvases: layout (`Rows`), drawing and hit-testing of every lane; one 180-line input function. |
| GPU | `player/gpu.rs`, `render.rs`, shaders | The shader stitch, used by the player and headless by the export. |

## What is wrong with it

1. **No API layer.** Edit operations are written inside iced message
   handlers, about 20 of them. Examples:
   - moving and trimming a sound clip;
   - adding, moving and deleting volume points (four copies);
   - overlay move, trim and key retime;
   - the index remap after a cut moves;
   - the free-look keyframe made on save.

   A CLI command or an AI agent can't call any of them, and they have no
   tests.
2. **`App` is the "one class".** One struct, one `Message` enum, one
   `handle()`. Selection is five loose `Option`s cleared by hand in a dozen
   places. Missed clears caused real bugs (fixed, below).
3. **The library depends on the binary and the binary on the GUI.**
   - `cache.rs`, `project.rs`, `lut.rs`, `check.rs`, `player/*` import
     `replace_file`, `Pipeline`, `StitchOpts`, `export` from `main.rs`.
   - The CLI's contact sheet uses `player::music` and the export uses
     `player::render`.
   - So there is no library a test, a second binary or an agent could link
     against.
4. **`Edit` copies `Project` field by field.** That means a 7-tuple
   snapshot, a 7-argument `replace`, `track()` written twice, a
   `to_project` and an open handler converting back and forth. Adding the
   SFX track touched ~750 lines in 10 files. A Voice track would touch
   ~26 places.
5. **Long functions:** `handle` 1100 lines, `view` 384, `Marks::update`
   180, `export` 155.
6. **Docs drifted.**
   - Module docs that still say "the music lane".
   - A stale `ffmpeg.rs` header ("a placeholder until in-process decode").
   - Four doc comments sitting on the wrong function.
   - The user guide knows nothing of sound lanes, Framing, normalize or
     the bin.

## Progress (2026-10-28, step 5 extended 2026-11-01)

| Step | State | Commits |
|---|---|---|
| 1. Library crate | done: `lib.rs`, `cli`, `pipeline`, `export`, `preview`, `analyse`, `fsutil`; export byte-identical | `de8167b` |
| 2. `Cut::new` | done | `f6cb3ac` |
| 3. `Edit` wraps `Project` | done: Deref to the project; `Project::as_shown` (save's view rule) tested | `cdd3a77` |
| 4. Split `player/mod.rs` | done: 4400 → 910 lines; `on_*` per area; `bars.rs` | `c96c636`, `3d91c99`, `67c9841`, `be71b78` |
| 5. Edit API | in part: volume points, `Music::drag_to`/`trim`, `Lane::move_to`/`trim`, `Edit::moved_index`, `Music::append`/`append_part`, `Edit::attempt`; one model for a cut's sound and a sound clip's (`project::Own`, `own_line`, `OwnSound`, `continue_level`, `before`, `own_file`), `Cut::set_sound_fade`, `fade_ramp`, `loudness::measure_stretch`; all tested | `5b28c58`, `a8bf813`, `8530506`, `9bc8478`, `23fee09` |
| 6. Sound tracks generic | done | `87a3f55` |
| 7. `ffmpeg` split | in part: `ffmpeg/mix.rs` with its test | `bb7f767` |
| 8. Timeline lanes | not started: changes drawing and the mouse everywhere, wants the user's eyes after | — |
| 9. Docs | the code's docs and CLAUDE.md's Layout; the user guide not yet | `e0b4bbf` |

Tests: 63 → 68 at the refactor, 106 by 2026-11-01 (`TESTING.md`). The
second pass's plan is `CORE-API.md`; its step 1 is done: the level rules
in one module, `src/mixdown.rs`. Still to do from step 5:
- one `Selection` enum instead of five loose `Option`s;
- overlay append and key retime;
- normalize through one function shared by the dialog and the CLI.

## Bugs the analysis found — fixed (2026-10-28)

| Commit | Bug |
|---|---|
| `f0c1786` | Save As panicked on an edit holding only sound effects. |
| `a822b63` | New and Open reset different, incomplete state; after Open, Del could delete an overlay of the new project. |
| `c352984` | A split that couldn't happen wiped redo and marked the edit unsaved; a split or insert left a sound point selected on the wrong cut. |
| `2e692d3` | A muted cut with a point above 0 dB played in the export (player: silent); player and export drew the −40 dB off-line differently. |
| `695f431` | `rosmo360 normalize` stopped at the first unreadable cut; «lower for voice-over» had no floor. |
| `1394b69` | Space didn't do what the Play button does; a drag on an item gone mid-drag made an undo step or indexed past the end. |
| `1690b75` | Adding a sound ran ffprobe on the UI thread (froze on slow drives) and added 60 s of nothing when it failed. |

Still open, minor: undo stacks are unbounded full clones; `audio_levels`
reads a whole file's PCM; NaN `in`/`out` pass `Project::load`'s check;
opening a project drops missing sounds and overlays silently from the file
on the next save.

## Where it is going

Five rules:
- **The model is plain data plus operations.** UI, CLI and an agent call
  the same operations.
- **Each area of the player is its own module.** `App` keeps the state
  struct and a thin dispatcher.
- **A library crate** (`src/lib.rs`) holds everything. `main.rs` is the
  CLI only.
- **Upstream's stitching and colour files stay untouched,** so they still
  merge.
- **Every step is a behaviour-free commit** that builds and passes the
  tests, except where it says it fixes something.

### Target layout

```
src/lib.rs            the crate: the modules below
src/main.rs           CLI entry only (clap → cli::run)
src/cli/              args (Cmd, StitchArgs), run, launch (Explorer), cache command
src/pipeline.rs       Pipeline, Shot, Carry, encode, grade_frame   (moved from main.rs unchanged)
src/export/           plan (pure: frames per cut, overlaps, handles), export
src/preview.rs        contact sheets
src/analyse/          info, scan, seam
src/fsutil.rs         replace_file, save_png
src/media/ffmpeg/     tool+probe, decode, encode, mix (the audio graph, unit-tested as strings)
src/model/            project, overlay, text (the file format), edit (undo), ops (the edit API)
src/gpu/              gpu, render, shaders (used headless by export; not player-only)
src/player/           the app, by area:
    mod.rs            App, Message, update dispatch, run
    file.rs           open, new, save, close, missing files
    transport.rs      tick, goto, crossfade partner, clip volume
    framing.rs        free look, view keyframes, Framing switch logic
    cuts.rs, tracks.rs, overlay_ui.rs, sound_ui.rs, jobs.rs (export, track, pick)
    bars.rs           top bar, transport row (view only)
    keys.rs           keyboard, framing_key
    icons.rs          SVG icons and small widget helpers
    timeline/         layout, lanes (one lane type per kind), input
```

### The edit API (`model::ops`)

Plain functions on the project, tested, used by the UI handlers, the CLI
and later the AI editor (`AUTO-EDIT.md`):

- **Cuts:** add at a spot, move (returns how indices moved), split, trim,
  crossfade, remove, set its sound (gain, mute, even, lower, restore),
  normalize (with the measuring injected, so tests need no ffmpeg).
- **Sound tracks:** append, move (with or without overlap), trim (the in
  edge keeps the song fixed on the edit), split, remove.
- **Volume points:** one implementation for a track's and a cut's (both
  `Vec<VolumeKey>`): add, move between neighbours, remove.
- **Overlays and text:** append, move, trim, retime a key.
- **Saving:** the hand-framed view held as a keyframe (`to_project`'s
  rule), so the CLI and an agent get the same file the UI writes.

### Order of work

Each step is one or a few commits:

1. **Library crate.** Add `lib.rs`; move `main.rs`'s non-CLI parts out
   unchanged (pipeline, export, preview, analyse, fsutil); move
   `gpu`/`render`/shaders and the waveform and envelope out of `player`.
2. **`Cut::new`.** One constructor instead of three 18-field literals.
3. **`Edit` wraps a `Project`.** The snapshot is the project, the track
   accessors live in one place, and the aspect becomes undoable.
4. **Split `player/mod.rs` by area,** as in the layout above. The view
   functions split first, then move.
5. **`model::ops` with tests,** one area per commit. The UI handlers then
   call them.
6. **Sound tracks iterated generically** (`SoundTrack::ALL`, a track's
   name, colour and placement on the enum), so a Voice lane is one
   variant.
7. **`ffmpeg` split, and tests of the mix graph.**
8. **Timeline lanes behind one lane type,** so a new lane is one
   implementation.
9. **Docs:**
   - module docs;
   - CLAUDE.md's Layout;
   - the user guide's sound, Framing, normalize and bin chapters;
   - `rosmo360 schema` and `describe` for agents (`AUTO-EDIT.md`).

**Cost to keep in mind.** After step 1 and step 4, upstream's `main.rs`
and `player/mod.rs` no longer merge cleanly. Upstream has been quiet since
2026-10-22 (0 commits ahead). From then on, an upstream change in those
files is ported by hand. The stitching and colour files are untouched and
still merge.
