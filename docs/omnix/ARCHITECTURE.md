# Omnix architecture — where things live, and where they are going

The map of the code as it is, and what's still to move. Rewrite it as
the code changes; the history is in git (the first refactor, gap 35,
2026-10-28; the core API, `CORE-API.md`, from 2026-11-01; the audit of
2026-11-05, worked off by 2026-11-07). Where each file lives: `AGENTS.md`, «Layout».

## The shape today

~36 000 lines of Rust in one crate, `omnix`: a library and a
six-line binary. Dailix (`../dailix`) links the library; the library
links Dailix's sheet core.

| Layer | Where | What it is |
|---|---|---|
| The core | `src/model/` | The project being edited (`Edit`: a `Project` with undo) and every change to it as a checked operation (`ops/`: cuts, clips, items, keys, sound), what's selected (one `Selection`). No iced, no ffmpeg. The player, `omnix apply`/`describe`/`ops-schema` and the tests all go through it. |
| The file format | `project.rs`, `overlay.rs`, `text.rs`, `grade.rs`, `view.rs` | Public JSON, backward compatible. |
| Transitions | `transition.rs` (+ `player/transition.wgsl`), `template.rs`, `dissolve.rs` | The looks as one CPU function and its shader twin, their knobs; templates' library; the export's joins and edge looks. |
| 360 and colour (upstream) | `stitch.rs`, `stab.rs`, `align.rs`, `exposure.rs`, `ring.rs`, `imu.rs`, `lut.rs`, `hdr.rs`, ... | The CPU reference; the shaders mirror it (AGENTS.md's pairs). |
| Media | `ffmpeg/`, `cache.rs`, `fsutil.rs` | ffmpeg as a separate program; the clip cache; safe writes and the app's folders. |
| Beside the editor | `desktop.rs`, `sibling.rs`, `from_sheet.rs` | The desktop entry (`omnix install`), finding and starting Dailix, an edit made from Dailix's marking. |
| Export and CLI | `cli.rs`, `pipeline.rs`, `export.rs`, `preview.rs`, `check.rs`, `analyse.rs` | The subcommands and the CPU render. |
| Sound rules | `mixdown.rs`, `loudness.rs`, `lufs.rs`, `duck.rs`, `mixcheck.rs`, `mixmeasure.rs` | Levels, loudness, the mix check: shared by player, export and CLI. |
| The player | `src/player/` | iced app: `App` and a thin dispatcher in `mod.rs`, an `on_*` handler per area in its own file; GPU drawing and export (`gpu.rs`, `render.rs`, shaders); decoding; the timeline (`timeline.rs`); the side panels (`transitions_panel.rs` with `transition_edges.rs` and `transition_templates.rs`, effects, audio, export, settings); `viewer.rs`, the player as a library for Dailix. |

## Rules it keeps

- **Every change to a project is an op** in `model::ops`, tested; a UI
  handler builds the op and applies it (inside `begin`/`commit` for a
  drag). A setting the CLI or an agent can't make is a bug.
- **Logic lives in a module with tests; the window only calls it.** A
  rule found in a message handler (what a key does, what to drop when a
  file is gone) moves to the model, pure where it can be.
- **One place for each fact**: a clock, a role's colour, the app's
  folders, a cut's media, a list of file types. The audit found most of
  the cost in copies that drifted apart, not in dead code.
- **A view shows the model**: the timeline's blocks hold the cut, clip
  or item they show; one `Hit` per canvas answers what's under the
  pointer; one `Picked` says what's selected. The habits behind the
  2026-11 cleanup, and their opposite: skill `code-rules`.
- **Upstream's stitching and colour files** are kept as they are; the
  rename (gap 76) ends merging from upstream anyway.

## Looked at and kept (audit of 2026-11-05)

The audit's list was worked off (the last of it the timeline package,
2026-11-07; the list itself is in git). These were looked at and left:

- `App::settings()` gathers ~33 fields from where they live (the bin,
  recording, the lanes' folds; the aspect and theme in other types) into
  what's saved: a projection, not a copy. One `settings` field would put
  `self.settings.…` in every file for no gain.
- The mute-aware level: a line each on `Cut`, `MusicClip`, `OwnSound`.
- The export using `player::render`: `gpu.rs` is an iced shader
  primitive and `render.rs` shares the player's decoding (`source.rs`);
  moving them to `src/gpu/` would rename a folder, not cut a dependency.
- The timeline's blocks hold a clone of the cut, clip or item they show,
  not a reference: a reference would carry a lifetime through all five
  canvases for no gain the user would see.
