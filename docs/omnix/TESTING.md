# Tests: what is covered, what to add

Gap 42 (user, 2026-11-01): "an analysis for unit and integration
testing; where we don't have such tests, add them, so refactoring and
improving happen more calmly". Analysis at `9381768`. The «Added» table
at the end follows the work; rewrite it, don't append below it.

## How tests run here

```sh
export PKG_CONFIG_PATH=$PWD/.localdeps/root/usr/lib/x86_64-linux-gnu/pkgconfig
~/.cargo/bin/cargo test --release                      # ~1 s, the live-mic test ignored
~/.cargo/bin/cargo test --release -- --include-ignored # with the microphone
```

- Most modules are private (`lib.rs` exports `cli`, `transition`,
  `from_sheet`, `desktop`, `sibling`, the viewer and a few folder
  helpers for Dailix), so unit tests live in `#[cfg(test)] mod tests`
  inside `src/`. `tests/cli.rs` drives the binary
  (`env!("CARGO_BIN_EXE_omnix")`).
- No coverage tool installed (no `llvm-cov`, no `tarpaulin`); the counts
  below are by hand.
- Media for tests is made at run time with `ffmpeg -f lavfi` (`testsrc2`,
  `sine`), tiny (320×180, ≤ 3 s), in a temp folder per test. Never real
  footage.
- **End to end works without footage:** two generated 3 s clips and a
  song, `omnix --no-cache export edit.json --cpu --size 320x180`:
  ~3.9 s, 105 frames (2 + 2 s at 30 fps less a 0.5 s crossfade),
  picture and sound both 3.500 s. `check` on it: ~3 s, 0 findings.
  Ordinary videos go through `Pipeline::load_flat`, no OSV needed.

## What has tests (count of `#[test]`, 2026-11-09)

200 in the library (199 run, the live microphone's ignored) and 10 end
to end in `tests/cli.rs`.

| File | Tests |
|---|---|
| `project.rs` | 30 |
| `model/edit.rs` | 15 |
| `player/timeline.rs` | 13 |
| `view.rs` | 8 |
| `model/ops/sound.rs`, `ffmpeg/mix.rs`, `duck.rs` | 7 each |
| `player/render.rs` (GPU vs CPU, transitions among them) | 6 |
| `model/ops/cuts.rs`, `grade.rs` | 5 each |
| `transition.rs`, `overlay.rs`, `model/selection.rs`, `model/ops/keys.rs`, `capture.rs` (1 ignored) | 4 each |
| `player/{sound_ui, keys, cuts, bin_view}.rs`, `notes.rs`, `lufs.rs`, `ffmpeg/mod.rs` | 3 each |
| `template.rs`, `dissolve.rs`, `fsutil.rs`, `cache.rs`, `lut.rs`, `loudness.rs`, `mixdown.rs`, `mixcheck.rs`, `model/ops/{props, clips}.rs`, `player/{marks, effects, bin}.rs`, `sky.rs`, `text.rs`, `track.rs` | 2 each |
| the rest with tests | 1 each |

**None:** `player/gpu.rs`, `cli.rs` (end to end only), `player/playback.rs`,
`align.rs`, `player/jobs.rs`, `pipeline.rs`, `player/bars.rs`,
`player/file.rs`, `player/framing.rs`, `export.rs` (end to end only), and
the smaller player modules.

## Riskiest untested logic

1. **Paths on load and save** — a bug, found by this analysis and
   reproduced (2026-11-01): a project opened by a relative path
   (`omnix normalize edit.json -o sub/n.json`, or `play edit.json`
   then Save As elsewhere) keeps its clips relative to the working folder
   (`Project::load` joins them to `path.parent()`, which is `""`), so the
   copy saved elsewhere names `a.mp4` and can't find it: `check
   sub/n.json` → «sub/a.mp4 has no video» (also a misleading message: the
   file is missing). Fix: make the folder absolute on load.
2. **The export's sound against the player's.** Mix tests only look for
   substrings; nothing runs the graph. `level_filter`'s nested `if`s are
   never checked against `volume_at`. Known divergence to pin down: a
   song cut off by the edit's end gets `END_FADE` *instead of* its own
   fade-out and no crossfade-out in the export, while the player takes
   the smaller of them.
3. **The export's frame plan** (`export.rs`): overlaps
   `round(crossfade·fps)` clamped, the mix length from frames while the
   timeline uses seconds; a frame of drift between picture, sound and
   fades would live here. Inline in `export()`, untestable today.
4. **`Dissolve`**: head and tail clamps, the temp-file swap, blend
   weights, the 16-bit path.
5. **`Music::fits` / `reach` / `place`**: a third clip in an overlap, a
   plain drag growing a crossfade, `reach` never returning an invalid
   value, `place` with no room.
6. **`offsets` / `locate` / `settle_crossfades` / `max_crossfade`**:
   negative `max_crossfade` on short cuts, `locate` exactly at joins,
   `remove(0)` zeroing the new first cut's crossfade.
7. **`Edit::moved_index`** at its edges.
8. **Volume points**: equal times, `move_volume_key` in a narrow range,
   `volume_key_at` within `SAME_POINT`.
9. **`continue_level` / `before`**: clips stored out of edit order, a
   muted one before.
10. **The cache's file format**: a truncated entry must read as none,
    not panic.
11. **`replace_file`**: symlinks followed, permissions kept, no temp
    file left on error.
12. **Overlay `Lane::place` / `room`**: three items packed tight.

## What blocks testing, and the smallest way out

- **The export's frame plan**: take `plan(...)` and `mix_length(...)`
  out of `export()` as pure functions.
- **`check`'s findings**: take the rules out as a function of the cuts
  plus the filmed times and pan speeds passed in.
- **Timeline hit-testing is testable now**: `Timeline`, `Block`,
  `MusicBlock`, `Rows` are plain data; `block_at`, `edge_hit`,
  `fade_at`, `sound_at`, `volume_y`/`volume_db` can be called on a
  hand-built `Rectangle`, no window.
- **App handlers**: `App::new` is free of side effects, but edits end in
  `goto`, which opens media. Test the handlers that don't reach it
  (the normalize dialog's states, `set_sound`, Lower/Restore,
  `DeletePoint`), or make `goto` return early with an empty library.
- **The crossfade shares** need a live playback: take
  `shares(edit, current, partner, t)` out.

## Integration tests

- **ffmpeg in lib tests** (as `loudness.rs` does): run the real
  `mixed()` graph on a generated tone and read levels back
  (`ffmpeg::audio_levels`); 0.3–1 s each. A shared
  `#[cfg(test)] mod testmedia` with `tone(path, secs, db)` and
  `clip(path, secs)` so the recipes aren't copied around.
- **The binary end to end, `tests/cli.rs`**: projects of generated clips,
  `--cpu --no-cache --size 320x180`, ~4 s per export; two or three of
  them, one export per test so they run side by side; skipped with a
  message if ffmpeg isn't on the PATH.
- **Old project files, `tests/fixtures/*.json`** (text only): one per
  shape the format has had (cuts only, `lut_strength`, music without SFX
  or Voice, overlays, `text`, `fades`, `sound`). `deny_unknown_fields`
  makes a rename break old files loudly; these catch it.

## Plan, in order

`[ff]` needs ffmpeg.

| # | Test | Where |
|---|---|---|
| 1 | Paths absolute after loading by a relative path; saved elsewhere, they still point at the files (with the fix) | `project.rs` |
| 2 | Save makes every kind of path relative under the folder, absolute outside it; load → save → load equal | `project.rs` |
| 3 | Old project files still load | `project.rs` + `tests/fixtures/` |
| 4 | `level_filter` run by ffmpeg matches `volume_at` within 0.5 dB [ff] | `mix.rs` |
| 5 | The music mix matches the player's envelope, crossfade and end cut-off [ff] | `mix.rs` |
| 6 | The export's frame plan (after taking it out) | `export.rs` |
| 7 | `Dissolve` blends head with tail, frame count, weights, no temp files left | `dissolve.rs` |
| 8 | CLI export end to end: frames, duration, sound under −1.2 dBFS [ff] | `tests/cli.rs` |
| 9 | CLI `--clips --handles`: a file per cut, the song's WAV [ff] | `tests/cli.rs` |
| 10 | CLI `check` flags a short cut and a repeat [ff] | `tests/cli.rs` |
| 11 | `fits` refuses a third clip in an overlap; a plain drag doesn't grow a crossfade | `project.rs` |
| 12 | `reach` stops at the last valid value | `project.rs` |
| 13 | `settle_crossfades` after remove and insert | `edit.rs` |
| 14 | `locate` and `edit_time` round trip | `edit.rs` |
| 15 | `moved_index` is a permutation matching `move_cut` | `edit.rs` |
| 16 | Volume point edge cases | `project.rs` |
| 17 | `continue_level` with clips out of order and a muted one | `project.rs` |
| 18 | Timeline hits: edges in a crossfade, fades, lines, `volume_db`/`volume_y` | `timeline.rs` |
| 19 | `Rows` layout is monotonic at every scale | `timeline.rs` |
| 20 | The cache rejects truncated entries | `cache.rs` |
| 21 | `replace_file` keeps permissions and the symlink | `fsutil.rs` |
| 22 | The normalize dialog runs and cancels, one undo step | `sound_ui.rs` |
| 23 | The cut mix graph with a crossfade and a silent clip | `mix.rs` |
| 24 | Fade bounds: zero length, fades longer than the clip | `project.rs` |

Pure tests for all but 4, 5 and 8–10, which make media.

## Added

The whole plan, 2026-11-01. Tests: 84 → 117 (113 in the library, 1 of
them the live microphone and ignored; 3 end to end). `cargo test
--release`: ~1 s for the library, ~8 s for the end-to-end ones.

| Plan | Test | Commit |
|---|---|---|
| 1 | `a_project_saved_elsewhere_still_finds_its_clips`, with the fix | `8ac74fa` |
| 2, 3 | `old_project_files_still_load` (`tests/fixtures/`), `save_names_paths_from_the_project_and_reads_back_the_same` | `8c47aee` |
| 4 | `the_exports_volume_line_sounds_as_the_player_draws_it` [ff] | `ac1f9e8` |
| 5 | `the_export_fades_sound_clips_as_the_player_does`, `the_export_cuts_a_clip_off_at_the_edits_end_as_the_player_does` [ff], with the fix | `562f65c` |
| 6 | `crossfades_share_frames_within_what_each_cut_has` (the frame plan taken out of `export()`) | `f63a2e1` |
| 7 | `the_outgoing_tail_blends_into_the_incoming_head`, `sixteen_bit_frames_blend_at_their_own_depth` | `ae825b7` |
| 8–10 | `tests/cli.rs`: export with crossfade and music, a file per cut with handles, `check` [ff] | `e6add7a` |
| 11, 12, 16, 17, 24 | sound clip placement, `reach`, volume points, `continue_level`, fades at their edges | `2988c09` |
| 13–15 | crossfades after remove and insert, edit time round trip, `moved_index` | `8491a7a` |
| 18, 19 | timeline rows at every scale; what a press finds (cuts, edges, fades, points, lines), with the fix | `03f85dc` |
| 20 | a cache entry cut short anywhere is a miss | `8a8f0a3` |
| 21 | `replace_file`: permissions, links, no temp file left | `227cb0d` |
| 22 | the Normalize dialog run whole without a window: one undo step, cancel, a track's clips | `e920fb3` |
| 23 | the cuts' sound graph: pad, crossfade, join, silence | `464b925` |
| — | auto-duck's speech finding and line (`duck.rs`), the level rules (`mixdown.rs`) | `4cdec41`, `7105eef` |

**Bugs the tests found, all fixed:**
- A project opened by a relative path, saved elsewhere, lost its clips
  (`8ac74fa`).
- Sound clips cut off by the edit's end sounded different in the export
  than in the player, up to 6 dB (`562f65c`).
- A level dragged near 0 dB snapped to 0 within 3 px: ±5.7 dB on the clip
  sound lane, so −3 dB couldn't be set there; now ±1 dB on any lane
  (`03f85dc`).

**Not covered, and why:** drawing (iced's canvas needs a window), the GPU
paths beyond `render::tests`' CPU comparisons, decoding and playback
(live ffmpeg streams and a sound device), the record panel's live take
(`records_a_take`, ignored: it needs a microphone).
