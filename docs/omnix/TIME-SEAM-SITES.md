# The time seam's sites (guard 3)

Every place that turns a cut's clip time into edit time or back, or takes
a duration in one clock for the other (inventory of 2026-11-12, about 195
sites; `TIME-SEAM.md` has the kinds and the steps). Line numbers drift:
the function is the anchor. A site is ticked with the commit that moved
it onto the seam; «31» marks media work left to card 31.

Kinds: **A** clip span as edit length · **B** clip → edit · **C** edit →
clip · **D** a duration's clock · **E** media at speed (card 31) · **F**
other 1:1 (frame grid, `MIN_CUT`, thumbnails, cache keys).

Already through a core function after guard 2 (`ea97943`): transport's
`follow_crossfade`, `crossfade_shares`, Scrub in an overlap; keys'
`key_volume_point`; cuts' `trim_to`. They move with the core in step 2.

## Core — step 2

| | Site | Kind |
|---|---|---|
| [x] af1f743 | `project.rs` `Cut::len` (the definition) | A |
| [x] step 1 | `project.rs` `offsets` | A D |
| [x] step 1 | `project.rs` `edit_length` | A |
| [x] af1f743 | `project.rs` `max_crossfade`, `settle_crossfades` | A D |
| [x] 23f3905 | `project.rs` `Cut::sound_fade`, `sound_fades` (fade vs edge seconds) | D |
| [x] 23f3905 | `project.rs` `Cut::edge_at` (`Edge.seconds` in clip time) | D |
| stays | `project.rs` `Cut::sound_span` (clip seconds of the file, by the table) | D |
| [x] af1f743 / 31 | `project.rs` `Cut::sound_beyond` (L placed at `at + edit_len`; the J/L stretch played at speed: card 31) | A B D |
| [x] 23f3905 | `project.rs` `Cut::settle_sound` (`MIN_CUT`) | F |
| [x] 23f3905 | `project.rs` `kept_picture` (edit length added to clip time) | A B C |
| 31 | `project.rs` `Project::sound_beyond` (places what `Cut::sound_beyond` gives) | B |
| [x] 23f3905 | `project.rs` `Project::black_at` | B |
| [x] 23f3905 | `project.rs` `OwnSound::set_fade` (clamped to the clip span) | D |
| [x] through offsets/edit_length | `project.rs` `duration`, `film_end`, `follow_groups`, `follow_locked`, `follow_from` (through helpers) | A |
| [x] step 1 | `model/edit.rs` `offset`, `locate`, `edit_time`, `clip_time`, `on_cut` | A B C |
| [x] step 1 | `model/edit.rs` `crossfade_from`, `crossfade_partner`, `crossfade_share` (each side at its own speed) | D |
| [x] 8f64bc1 | `model/edit.rs` `split`, `trim` (`MIN_CUT` on clip spans) | F |
| [x] af1f743 | `model/edit.rs` `set_crossfade` (through `max_crossfade`) | D |
| [x] af1f743 | `model/edit.rs` `drawn_zone` (edit span → crossfade or edge seconds) | A D |
| [x] 8f64bc1 | `model/ops/cuts.rs` `edge` (`seconds.min(end − start)`) | A D |
| [x] 8f64bc1 | `model/ops/cuts.rs` add (a new cut is 1×), split, `sound_edge` (`MIN_CUT`) | F |
| [x] 8f64bc1 | `model/ops/mod.rs` `check` (sound span ≥ `MIN_CUT`) | F |
| [x] 8f64bc1 | `model/ops/auto.rs` `follow` (track samples to edit time) | B |

## Timeline — step 3

| | Site | Kind |
|---|---|---|
| [x] 6cbda6b | `Block::len` (its own copy of `Cut::len`, 31 uses) | A |
| [x] 6cbda6b / 31 | `Block::sound_span` | A D |
| [x] 6cbda6b | `block_at`, `edge_at`, `block_x`, `drop_at`, `level_tag_at` | A |
| [x] 6cbda6b | `key_at`, `frame_key_at`, `draw_frame_key`, keyframe diamonds, `draw_sound`, `draw_own_line` | B |
| [x] 6cbda6b | `effect_at`, effect marks, `flags`, `flags_height`, `sound_at` | A B |
| [x] 6cbda6b | `join_at`, crossfade X on both lanes | D |
| [x] 6cbda6b | `readout` (Trim length), `draw_cut_marks` | A |
| [x] 0e6975e | `cut_fade_at`, `draw_fades` (both), `draw_clip_sound` (a cut at speed has no sound past its picture: its span is the picture's) | D F |
| [x] 0e6975e | `own_place` (`OwnPlace`), `sound_x` | D |
| [x] 0e6975e | `on_sound_line`, `own_point` | C |
| [x] 6cbda6b | edge wedges, `zones` | A D |
| [x] 6cbda6b | `on_frame` (rounds clip frames to edit time) | C F |
| [x] 6cbda6b | `snap_targets` (cut ends, sound edges, markers) | A B D |
| [x] 6cbda6b | sounds cache key (hash speed too) | F |
| [x] 6cbda6b | thumbnail strip (`per_px` from `len`) | A F |
| [x] 0e6975e | `Wave::draw` (edit length wide, the file read through the speed) | F |
| [x] 6cbda6b | presses: Trim, Fade, SoundEdge | A D |
| [x] 6cbda6b | drags: Key, Effect, Trim, SoundEdge, FrameKey (`drag_frame_key`) | A B C |
| [x] 6cbda6b | drags: Crossfade, EdgeLength, Fade | D |
| [x] 6cbda6b | `timeline/auto.rs` `x`, `own_time`, own-edge snap, clips in view, reach, body-move clamp, fixed bars, `Owner` built from `Block` | A B C |

## Player — step 4

| | Site | Kind |
|---|---|---|
| [x] 2a4d453 | `transport.rs` `follow_music`, `position` (through `edit_time`) | B |
| [x] 2a4d453 | `transport.rs` `play_stretch`, `after_edit` (through `locate`) | C |
| [x] 2a4d453 | `transport.rs` crossfade volumes, `cut_level`, `change` | D |
| [x] cbfd6bb | `transport.rs` `carries_on` (`Media::plays`: equal speeds) | F |
| [x] 0646a0a | `transport.rs` `PREROLL`, sound lead, catch-up: edit seconds at the cut's speed | D 31 |
| [x] stays | `transport.rs` `follow_stretch`, `fps`, `frame_len`, toggle at the end: edit time already | F |
| [x] stays | `keys.rs` `step_frame`, `seek_to`, `aimed`: a step is one clip frame | B C F |
| [x] 2a4d453 | `keys.rs` `stops` | A B |
| [x] 2a4d453 | `effects.rs` `focus`, `effect_focus`, `go_to`, key gap | A B C F |
| [x] 2a4d453 | `marks.rs` `MarkSelect` | B C |
| [x] 2a4d453 | `cuts.rs` `cut_labels` (length shown), Trim / TrimToPlayhead (`frame_len`) | A F |
| [x] 2a4d453 | `transitions_panel.rs` length steps, `join_window`, crossfade shown | D |
| [x] 2a4d453 | `info.rs` «On the edit» | A |
| [x] 2a4d453 | `framing.rs` `grade`, `sphere` (edge and style at clip `t`) | B D |
| [x] 2a4d453 | `bars.rs` `timeline_data`, `can_split` | A F |
| [x] 2a4d453 | through helpers only: `overlay_ui`, `record_ui`, `bin`, `groups`, `bin_player`, `file`, `jobs`, `sound_ui` | — |

## Agents and CLI — step 5

| | Site | Kind |
|---|---|---|
| [x] cf9f68e | `apply.rs` `describe` (cut span, markers, crossfade text) | A B D |
| [x] cf9f68e | `check.rs` short cuts, pan per second | A B F |
| [x] cf9f68e | `preview.rs` tile times, bands, `frame_at` | A B C F |
| [x] 0e6975e | `duck.rs` `sources`, `speech` (`Source.speed`) | B |
| [x] cf9f68e / 31 | `mixmeasure.rs` `cut_sounds`, windows, items | A E |

`check` and `preview` read media, so the snapshots don't cover them:
each gets a test of its times before it moves.

## Export — step 6 (and card 31)

| | Site | Kind |
|---|---|---|
| [x] 3116c26 | `export.rs` `cut_frames` (frames = clip span × fps) | A E |
| [x] 3116c26 | `export.rs` `overlaps`, `edit_seconds` | D A |
| [x] 0c0f0e4 | `export.rs` `stretches` (edit seconds; `Sound::reads` the clip span) | E D |
| [x] 3116c26 / 31 | `export.rs` `overs`, `camera` (`t = start + k/fps`) | B E |
| [x] 3116c26 | `export.rs` handles (clip seconds) | D |
| [x] 0c0f0e4 | 31: `pipeline.rs` `encode`, `player/render.rs` `encode` (`Reading`: frames picked by time) | E |
| [x] 0c0f0e4 | 31: `ffmpeg/mod.rs` `sound_inputs`, `sound_stream`, `Encoder::new` | E |
| [x] 0c0f0e4 | 31: `ffmpeg/mix.rs` `level_filter`, `afade`, `acrossfade`, `pad`, `Sound::of_cut` (tempo before the pad) | D E |

## Media — card 31 only

| | Site | Kind |
|---|---|---|
| [x] 0646a0a | `playback.rs` `now` (clip time at the speed), `start_audio` (`filter_at`); `step`, `snap` stay clip frames | E F |
| [x] stays | `source.rs` `spawn`, `pump` (pts): clip time, the frames due shown | E |
| [x] 0646a0a | `audio.rs` `prepare` (the tempo in its filter), `elapsed` (edit seconds heard) | E |
| [ ] 31 | `music.rs` `MusicPlayer::follow` (J/L sound at 1×: a speed is refused on a J- or L-cut) | E |
| [x] 0646a0a | `media.rs` `Media::for_cut` (its speed), `plays` | E |
| [x] 0646a0a | `transport.rs` partner priming, `sound_lead`, `p.now() < cut.end` | E |
