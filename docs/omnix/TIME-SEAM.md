# The time seam (2026-11-12, analysis before code)

Step 1 of `PLAN-NEXT.md`. A cut reads its clip from `start` to `end`
(clip time); the edit places it at `offsets[i]` (edit time). Today one
clip second is one edit second everywhere, and that is written by hand.
Speed (31, 31c) breaks every place that relies on it. The seam puts the
conversion behind a few functions first, at speed 1, so nothing visible
changes and later work (paste, stingers, speed) uses it from day one.

## How big it is

The earlier estimate (`offsets(` ×23, `len()` ×50, «about 70») was low. A
full read of `src/` found **about 195 sites**; Dailix has none (it builds
cuts from the sheet in clip seconds and never times them).

| Kind | What the site does | Sites | In the seam? |
|---|---|---|---|
| A | takes a cut's clip span as its length on the edit | ~70 | yes |
| B | clip time → edit time (keys, marks, volume points, frame keys, beats) | ~40 | yes |
| C | edit time → clip time (playhead, scrub, drags, trims, sync lock) | ~30 | yes |
| D | a duration whose clock is unclear (crossfade, edge seconds, fades, J/L) | ~50 | yes, after a decision |
| E | reads media 1:1 (player clock, audio, decoder, ffmpeg `-ss/-t`, mix) | ~35 | **no — card 31** |
| F | other 1:1 assumptions (frame grid, `MIN_CUT`, thumbnails, cache keys) | ~30 | mostly yes |

By file: `player/timeline.rs` ~70, `player/transport.rs` ~30, `project.rs`
~22, `model/edit.rs` ~12, `player/timeline/auto.rs` 10, `ffmpeg/` ~12,
`export.rs` ~11, `preview.rs` 8, `player/keys.rs` 6, `mixmeasure.rs` 4,
`check.rs` 4.

## What exists and what goes around it

- `Cut::len` (15 callers) — but the timeline has its own `Block::len`
  (31 uses) and `auto.rs` its `Owner.len`; ~6 more places write
  `end - start` by hand (`preview.rs:76`, `info.rs:247`, `cuts.rs:16`,
  `ops/cuts.rs:398`, …).
- `Project::edit_time` (7 callers) — ~35 sites write `from + t - start`
  by hand, ~20 of them in `timeline.rs` + `auto.rs`.
- `Project::locate` (11 callers) — ~8 sites write edit→clip by hand:
  `transport.rs` Scrub in an overlap, `keys.rs` `key_volume_point`,
  `cuts.rs` `trim_to`, `effects.rs` `focus`, the timeline's `own_point`,
  `on_sound_line`, `own_time` and its drags.
- `crossfade_from` (3 callers) — the rest of the crossfade maths
  (`follow_crossfade`'s incoming branch, `crossfade_shares`) is by hand.
- `black_at`, `sound_span`, `sound_fades`, `sound_beyond`, `edge_at` —
  the places where the D decisions land.
- `MusicClip::local` / `Item::local` — the shape to copy for cuts.

## The decision the seam needs: which clock each stored value is in

Speed 1 hides it; at 2× the two clocks differ, so each stored value must
be one or the other. Proposed rule: **what belongs to the picture's
content is clip time; what the viewer sees as a length on the film is
edit time.**

| Value | Clock | Why |
|---|---|---|
| `start`, `end` | clip | where in the file |
| keys, marks, volume points, automation, frame keys, effect keys | clip | tied to what happens in the picture: a key on the scooter stays on the scooter when the cut speeds up |
| `sound_before` / `sound_after` (J/L) | clip | a span of the file's sound, read at the cut's speed |
| `crossfade` | **edit** | the edit is that much shorter (as documented); each side converts it with its own speed |
| `Edge.seconds` (edge looks) | **edit** | drawn, dragged and offered as a length on the film |
| `sound_fade_in` / `_out` | **edit** | a fade is heard on the film |
| `MIN_CUT` | **edit** | the shortest thing the viewer sees |

Today `edge_at`, `sound_fades`, `max_crossfade`, `crossfade_from`,
`follow_crossfade`, export `overlaps` and ffmpeg `acrossfade` read these
durations as clip seconds of both cuts at once. That is wrong at two
different speeds even though nothing shows it now.

## The proposed API

On `Cut` (with `speed()` = 1 for now):

- `edit_len()` — its length on the edit; replaces `len()` wherever the edit
  is meant (`len()` stays for the clip span, renamed `clip_len()` to stop
  the two being confused).
- `edit_at(clip_t) -> f64` — seconds from its in point on the edit.
- `clip_at(edit_dt) -> f64` — clip time that far into it on the edit.
- `clip_span(edit_secs, end)` — the clip stretch an edit-time duration
  covers at its head or tail (crossfades, edge looks, fades).

On `Project`: `offsets`, `edit_length`, `locate`, `edit_time`,
`crossfade_from`, `black_at` rewritten over these. The timeline's `Block`
and `auto.rs`'s `Owner` get `x_of(clip_t)` / `clip_of(x)` instead of
`from + t - start`; drags turn pixels into **edit** seconds, then into clip
time (today a trim at 4× would move 4× too slowly).

To test the seam at speed ≠ 1 before card 31 exists: a `Speed` type (a
curve; one key is a constant, as `PLAN-NEXT.md` asks) and a `Cut::speed`
field, `serde(default)`, not reachable from ops, the panel or `describe`
yet. The pure functions are tested at 0.5× and 2×; every other test
stays at 1× and must not change.

## What stays for card 31 (kind E)

Not the seam: the player clock (`playback.rs` `now`, `audio.rs`
`elapsed`), audio tempo (`audio.rs` `prepare`, `atempo`/`asetrate`), the
decoder's pts (`source.rs`), the music player playing J/L beyond clips at
1× (`music.rs`, `sound_beyond` → plain `MusicClip`), ffmpeg sound inputs
(`ffmpeg/mod.rs` `-ss/-t`), the mix's `level_filter` / `afade` /
`acrossfade` positions, the export's frame loop (`pipeline.rs`,
`render.rs`: one decoded frame per output frame), and loudness windows
(`mixmeasure.rs`). The seam gives them correct times; making the media
actually run at speed is 31's work.

## Riskiest places

1. Player clock and `tick` (`p.now() >= cut.end`), preroll, partner sync.
2. Crossfade meaning across two speeds (see the table).
3. Export frame plan: frames = clip span × fps; `Sound::of_cut` gets
   `frames/fps` as a clip `-t`. Picture and sound would drift silently.
4. Sync lock (`kept_picture`, `follow_picture`): mixes edit lengths and clip
   spans (`b.start + (owned_to - old[k])`) — would cut locked music in the
   wrong place without any error.
5. J/L `sound_beyond`: clip seconds placed as edit seconds, played at 1×.
6. Edge looks and fades: `Edge.seconds` is clip time in `edge_at`, edit
   time in the timeline, `drawn_zone`, `join_window`.
7. Timeline drags (Trim, SoundEdge, FrameKey, Key, Effect, `on_frame`).
8. The hand-written conversions listed above.
9. The frame grid (`on_frame`, `snap`, `step_frame`, `frame_len`,
   `1.5/fps` end tests): edit frames and clip frames stop lining up.
10. Loudness windows (`mixmeasure.rs`) against the export.

## Steps (one local commit each)

1. `Speed` + the `Cut` functions + `clip_len` rename, tested at 0.5×/2×.
2. Core: `offsets`, `edit_length`, `locate`, `edit_time`, `crossfade_from`,
   `max_crossfade`, `black_at`, `edge_at`, `sound_fades`, `sound_beyond`
   placement, `kept_picture` / `follow_picture`, `drawn_zone`, `MIN_CUT`
   checks in ops. Sync-lock tests at 2×.
3. Timeline: `Block` / `Owner` over the seam, drags through edit seconds,
   snap targets, zones, thumbnails, the sound cache key.
4. Player: transport (scrub, crossfade shares, `follow_crossfade`), keys,
   effects `focus`, cuts `trim_to`, info, labels.
5. Agents and CLI: `describe`, `check`, `preview`, `duck`, `ops/auto`
   follow, `mixmeasure` windows.
6. Export: the frame plan and the per-frame clip time through the seam
   (the decode itself stays 1:1 until 31).
7. A guard test: no `end - start` / `+ t - start` on a cut outside the
   seam (a source scan), so new code can't go around it.

Every step: all 336 tests pass unchanged at 1×; the user's check is that
nothing changed (play, trims, crossfades, J/L, sync lock, export).
