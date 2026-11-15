# The time seam's sites (guard 3)

Every place that turns a cut's clip time into edit time or back, or takes
a duration in one clock for the other (inventory of 2026-11-12, about 195
sites; `TIME-SEAM.md` has the kinds and the steps). Line numbers drift:
the function is the anchor. A site is ticked with the commit that moved
it onto the seam; «31» marks media work left to card 31.

Kinds: **A** clip span as edit length · **B** clip → edit · **C** edit →
clip · **D** a duration's clock · **E** media at speed (card 31) · **F**
other 1:1 (frame grid, `MIN_CUT`, thumbnails, cache keys).

Already through a core function after guard 2 (`84ae21c`): transport's
`follow_crossfade`, `crossfade_shares`, Scrub in an overlap; keys'
`key_volume_point`; cuts' `trim_to`. They move with the core in step 2.

## Core — step 2

| | Site | Kind |
|---|---|---|
| [x] b3db0a8 | `project.rs` `Cut::len` (the definition) | A |
| [x] step 1 | `project.rs` `offsets` | A D |
| [x] step 1 | `project.rs` `edit_length` | A |
| [x] b3db0a8 | `project.rs` `max_crossfade`, `settle_crossfades` | A D |
| [x] ba7b2f0 | `project.rs` `Cut::sound_fade`, `sound_fades` (fade vs edge seconds) | D |
| [x] ba7b2f0 | `project.rs` `Cut::edge_at` (`Edge.seconds` in clip time) | D |
| stays | `project.rs` `Cut::sound_span` (clip seconds of the file, by the table) | D |
| [x] b3db0a8 / 31 | `project.rs` `Cut::sound_beyond` (L placed at `at + edit_len`; the J/L stretch played at speed: card 31) | A B D |
| [x] ba7b2f0 | `project.rs` `Cut::settle_sound` (`MIN_CUT`) | F |
| [x] ba7b2f0 | `project.rs` `kept_picture` (edit length added to clip time) | A B C |
| 31 | `project.rs` `Project::sound_beyond` (places what `Cut::sound_beyond` gives) | B |
| [x] ba7b2f0 | `project.rs` `Project::black_at` | B |
| [x] ba7b2f0 | `project.rs` `OwnSound::set_fade` (clamped to the clip span) | D |
| [x] through offsets/edit_length | `project.rs` `duration`, `film_end`, `follow_groups`, `follow_locked`, `follow_from` (through helpers) | A |
| [x] step 1 | `model/edit.rs` `offset`, `locate`, `edit_time`, `clip_time`, `on_cut` | A B C |
| [x] step 1 | `model/edit.rs` `crossfade_from`, `crossfade_partner`, `crossfade_share` (each side at its own speed) | D |
| [x] 5e0e4b5 | `model/edit.rs` `split`, `trim` (`MIN_CUT` on clip spans) | F |
| [x] b3db0a8 | `model/edit.rs` `set_crossfade` (through `max_crossfade`) | D |
| [x] b3db0a8 | `model/edit.rs` `drawn_zone` (edit span → crossfade or edge seconds) | A D |
| [x] 5e0e4b5 | `model/ops/cuts.rs` `edge` (`seconds.min(end − start)`) | A D |
| [x] 5e0e4b5 | `model/ops/cuts.rs` add (a new cut is 1×), split, `sound_edge` (`MIN_CUT`) | F |
| [x] 5e0e4b5 | `model/ops/mod.rs` `check` (sound span ≥ `MIN_CUT`) | F |
| [x] 5e0e4b5 | `model/ops/auto.rs` `follow` (track samples to edit time) | B |

## Timeline — step 3

| | Site | Kind |
|---|---|---|
| [ ] | `Block::len` (its own copy of `Cut::len`, 31 uses) | A |
| [ ] | `Block::sound_span` | A D |
| [ ] | `block_at`, `edge_at`, `block_x`, `drop_at`, `level_tag_at` | A |
| [ ] | `key_at`, `frame_key_at`, `draw_frame_key`, keyframe diamonds, `draw_sound`, `draw_own_line` | B |
| [ ] | `effect_at`, effect marks, `flags`, `flags_height`, `sound_at` | A B |
| [ ] | `join_at`, crossfade X on both lanes | D |
| [ ] | `readout` (Trim length), `draw_cut_marks` | A |
| [ ] | `cut_fade_at`, `draw_fades` (both), `draw_clip_sound` | D F |
| [ ] | `own_place`, `sound_x` | D |
| [ ] | `on_sound_line`, `own_point` | C |
| [ ] | edge wedges, `zones` | A D |
| [ ] | `on_frame` (rounds clip frames to edit time) | C F |
| [ ] | `snap_targets` (cut ends, sound edges, markers) | A B D |
| [ ] | sounds cache key (hash speed too) | F |
| [ ] | thumbnail strip (`per_px` from `len`) | A F |
| [ ] | `Wave::draw` (`len` as width and as seconds) | F |
| [ ] | presses: Trim, Fade, SoundEdge | A D |
| [ ] | drags: Key, Effect, Trim, SoundEdge, FrameKey (`drag_frame_key`) | A B C |
| [ ] | drags: Crossfade, EdgeLength, Fade | D |
| [ ] | `timeline/auto.rs` `x`, `own_time`, own-edge snap, clips in view, reach, body-move clamp, fixed bars, `Owner` built from `Block` | A B C |

## Player — step 4

| | Site | Kind |
|---|---|---|
| [ ] | `transport.rs` `follow_music`, `position` (through `edit_time`) | B |
| [ ] | `transport.rs` `play_stretch`, `after_edit` (through `locate`) | C |
| [ ] | `transport.rs` crossfade volumes, `cut_level`, `change` | D |
| [ ] | `transport.rs` `carries_on` (also needs equal speeds) | F |
| [ ] | `transport.rs` `PREROLL` in clip seconds | D 31 |
| [ ] | `transport.rs` `follow_stretch`, `fps`, `frame_len`, toggle at the end | F |
| [ ] | `keys.rs` `step_frame`, `seek_to`, `aimed` | B C F |
| [ ] | `keys.rs` `stops` | A B |
| [ ] | `effects.rs` `focus`, `effect_focus`, `go_to`, key gap | A B C F |
| [ ] | `marks.rs` `MarkSelect` | B C |
| [ ] | `cuts.rs` `cut_labels` (length shown), Trim / TrimToPlayhead (`frame_len`) | A F |
| [ ] | `transitions_panel.rs` length steps, `join_window`, crossfade shown | D |
| [ ] | `info.rs` «On the edit» | A |
| [ ] | `framing.rs` `grade`, `sphere` (edge and style at clip `t`) | B D |
| [ ] | `bars.rs` `timeline_data`, `can_split` | A F |
| [ ] | through helpers only: `overlay_ui`, `record_ui`, `bin`, `groups`, `bin_player`, `file`, `jobs`, `sound_ui` | — |

## Agents and CLI — step 5

| | Site | Kind |
|---|---|---|
| [ ] | `apply.rs` `describe` (cut span, markers, crossfade text) | A B D |
| [ ] | `check.rs` short cuts, pan per second | A B F |
| [ ] | `preview.rs` tile times, bands, `frame_at` | A B C F |
| [ ] | `duck.rs` `sources`, `speech` | B |
| [ ] | `mixmeasure.rs` `cut_sounds`, windows, items | A E |

`check` and `preview` read media, so the snapshots don't cover them:
each gets a test of its times before it moves.

## Export — step 6 (and card 31)

| | Site | Kind |
|---|---|---|
| [ ] | `export.rs` `cut_frames` (frames = clip span × fps) | A E |
| [ ] | `export.rs` `overlaps`, `edit_seconds` | D A |
| [ ] | `export.rs` `stretches` (`frames/fps` as a clip `-t`) | E D |
| [ ] | `export.rs` `overs`, `camera` (`t = start + k/fps`) | B E |
| [ ] | `export.rs` handles (clip seconds) | D |
| [ ] | 31: `pipeline.rs` `encode`, `player/render.rs` `encode` (a decoded frame per output frame) | E |
| [ ] | 31: `ffmpeg/mod.rs` `sound_inputs`, `sound_stream`, `Encoder::new` | E |
| [ ] | 31: `ffmpeg/mix.rs` `level_filter`, `afade`, `acrossfade`, `pad`, `Sound::of_cut` | D E |

## Media — card 31 only

| | Site | Kind |
|---|---|---|
| [ ] | `playback.rs` `now`, `hear`, `start_audio`, `start_stream`, `jump`, `step`, `snap` | E F |
| [ ] | `source.rs` `spawn`, `pump` (pts) | E |
| [ ] | `audio.rs` `prepare` (no tempo), `elapsed` (the clock) | E |
| [ ] | `music.rs` `MusicPlayer::follow` (J/L sound at 1×) | E |
| [ ] | `media.rs` `Media::for_cut` | E |
| [ ] | `transport.rs` partner priming, `sound_lead`, `p.now() < cut.end` | E |
