# Code audit, 2026-11-05: what's left to clean

Five read-only passes over Omnix and Dailix (dead code, code that does
nothing, duplicates, over-engineering, tangled logic, stale docs), each
finding checked before it's fixed. A finding leaves this page when its
commit lands; the page goes when the list is empty. Strictly dead code
turned out rare (Dailix's sheet core had some, gone in `1573719`); the
cost is duplication and drift.

## Bugs found on the way (first)

- [ ] Timeline: press and hover test what's under the pointer in different
      orders (level tag vs fade knob): the cursor lies (`timeline.rs`
      `Marks::update` / `mouse_interaction`; `OverlayLane` too).
- [ ] Edit fades, clip-sound level, track level: no `Op`, set directly
      (`effects.rs`, `tracks.rs`).
- [ ] `rosmo360 duck` saves directly; `SoundOp::Duck` exists (`duck.rs`).
- [ ] Opening a project drops missing files in a UI handler: a tested
      `Project::drop_missing` (`file.rs`).
- [ ] Dailix: I/O/M/N/PageUp-Down marking logic in `pane_key` (100 lines,
      untested): a pure `marking.rs` returning ops.
- [ ] Dailix: «→ Omnix» lookup and launch in `update`: an `omnix.rs`.
- [ ] Contact-sheet segment writes ("sheet, else here only") ×3 in
      `bin_player.rs`; `Remove` uses commit/begin by hand, not `self.ops`.

## Duplicates

Across the two repos:

In Omnix:
- [ ] Modal shade ×4; job thread scaffolding ×2; `note()` ×2.
- [ ] A cut's media built ×3 (`transport.rs`); cut level formula ×2.
- [ ] `request_still` thread ×2; mono PCM decode ×2 (`music.rs`, ffmpeg).
- [ ] Info read and drawn twice (bin side pane, Info panel).
- [ ] File type lists disagree (Open dialog vs `CLIP_EXT`).
- [ ] Whole-film `Mix` assembly ×3; `Sound` spelt out by hand.
- [ ] Find-by-id ×7; `Lane::place` = `Music::place`; mute-aware level ×3.
- [ ] Loading `Options` literal ×3; "is the edit playing" ×9; pass-through
      wrappers (`cut_wave_file`, `Media::filter`, `now`).
- [ ] Timeline per-lane code (edges, fades, blocks, folded strips,
      crossfade X, lane boxes) copied across lanes; view structs copy the
      model field by field; five selection fields rebuilt from one.

## Tangled

- [ ] `App` mirrors ~33 settings by hand (in at `new`, out at `settings()`).
- [ ] `Marks::update` 241 lines; `export()` 159; `cli::run` 157 (preview
      and export logic inline); Dailix `update` 243.
- [ ] Library modules reach into `player` and `cli` (`Waveform`,
      `render`, `osv_under`).
- [ ] Tests in the wrong module (`sound_ui.rs` tests cuts and markers).

## Help text in panels (house rule)

- [ ] Audio and Export panels' paragraphs → tooltips. Settings' notes were
      accepted by the user; they stay.

## For the user to decide

- Dailix `Edit` (undo): built, tested, used by nothing; its snapshots
  would overwrite another writer's change. Drop it, or build undo as
  inverse ops through `Store` with a key.
- Two LUFS meters (ffmpeg `ebur128` and our own, agreeing within 0.3 LU):
  one would do, but stored gains would shift slightly on a re-run.
- Upstream maths copies (equirect direction ×5, vector helpers ×3): merge
  them unless upstream merges are still wanted (the rename says no).
- `check` parses stitch flags it ignores; IMU flags threaded through the
  player though AGENTS.md fixes their values: trim the CLI?

## Stale docs

- [ ] AGENTS.md Layout (main.rs, ffmpeg.rs, missing modules).
- [ ] ARCHITECTURE.md shape table and steps; CORE-API.md names.
- [ ] timeline.rs header (seek bar, two lanes), misplaced doc comments
      (timeline, icons, framing, bars, effects, keys, jobs, cuts, tracks,
      mod, thumbs, overlay, pipeline, loudness).
