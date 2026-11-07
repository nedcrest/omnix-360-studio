# Omnix architecture — where things live, and where they are going

The map of the code as it is, and what's still to move. Rewrite it as
the code changes; the history is in git (the first refactor, gap 35,
2026-10-28; the core API, `CORE-API.md`, from 2026-11-01; the audit of
2026-11-05, `AUDIT.md`). Where each file lives: `AGENTS.md`, «Layout».

## The shape today

~33 000 lines of Rust in one crate, `omnix`: a library and a
six-line binary. Dailix (`../dailix`) links the library; the library
links Dailix's sheet core.

| Layer | Where | What it is |
|---|---|---|
| The core | `src/model/` | The project being edited (`Edit`: a `Project` with undo) and every change to it as a checked operation (`ops/`: cuts, clips, items, keys, sound), what's selected (one `Selection`). No iced, no ffmpeg. The player, `omnix apply`/`describe`/`ops-schema` and the tests all go through it. |
| The file format | `project.rs`, `overlay.rs`, `text.rs`, `grade.rs`, `view.rs` | Public JSON, backward compatible. |
| 360 and colour (upstream) | `stitch.rs`, `stab.rs`, `align.rs`, `exposure.rs`, `ring.rs`, `imu.rs`, `lut.rs`, `hdr.rs`, ... | The CPU reference; the shaders mirror it (AGENTS.md's pairs). |
| Media | `ffmpeg/`, `cache.rs`, `fsutil.rs` | ffmpeg as a separate program; the clip cache; safe writes and the app's folders. |
| Export and CLI | `cli.rs`, `pipeline.rs`, `export.rs`, `preview.rs`, `check.rs`, `analyse.rs` | The subcommands and the CPU render. |
| Sound rules | `mixdown.rs`, `loudness.rs`, `lufs.rs`, `duck.rs`, `mixcheck.rs`, `mixmeasure.rs` | Levels, loudness, the mix check: shared by player, export and CLI. |
| The player | `src/player/` | iced app: `App` and a thin dispatcher in `mod.rs`, an `on_*` handler per area in its own file; GPU drawing and export (`gpu.rs`, `render.rs`, shaders); decoding; the timeline (`timeline.rs`); `viewer.rs`, the player as a library for Dailix. |

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
- **Upstream's stitching and colour files** are kept as they are; the
  rename (gap 76) ends merging from upstream anyway.

## What's still to move (from `AUDIT.md`)

1. **Long functions:** `Marks::update` 241 lines (after 1).
