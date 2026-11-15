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
| [x] c0dd9ad | `Block::len` (its own copy of `Cut::len`, 31 uses) | A |
| [x] c0dd9ad / 31 | `Block::sound_span` | A D |
| [x] c0dd9ad | `block_at`, `edge_at`, `block_x`, `drop_at`, `level_tag_at` | A |
| [x] c0dd9ad | `key_at`, `frame_key_at`, `draw_frame_key`, keyframe diamonds, `draw_sound`, `draw_own_line` | B |
| [x] c0dd9ad | `effect_at`, effect marks, `flags`, `flags_height`, `sound_at` | A B |
| [x] c0dd9ad | `join_at`, crossfade X on both lanes | D |
| [x] c0dd9ad | `readout` (Trim length), `draw_cut_marks` | A |
| [x] 9283654 | `cut_fade_at`, `draw_fades` (both), `draw_clip_sound` (a cut at speed has no sound past its picture: its span is the picture's) | D F |
| [x] 9283654 | `own_place` (`OwnPlace`), `sound_x` | D |
| [x] 9283654 | `on_sound_line`, `own_point` | C |
| [x] c0dd9ad | edge wedges, `zones` | A D |
| [x] c0dd9ad | `on_frame` (rounds clip frames to edit time) | C F |
| [x] c0dd9ad | `snap_targets` (cut ends, sound edges, markers) | A B D |
| [x] c0dd9ad | sounds cache key (hash speed too) | F |
| [x] c0dd9ad | thumbnail strip (`per_px` from `len`) | A F |
| [x] 9283654 | `Wave::draw` (edit length wide, the file read through the speed) | F |
| [x] c0dd9ad | presses: Trim, Fade, SoundEdge | A D |
| [x] c0dd9ad | drags: Key, Effect, Trim, SoundEdge, FrameKey (`drag_frame_key`) | A B C |
| [x] c0dd9ad | drags: Crossfade, EdgeLength, Fade | D |
| [x] c0dd9ad | `timeline/auto.rs` `x`, `own_time`, own-edge snap, clips in view, reach, body-move clamp, fixed bars, `Owner` built from `Block` | A B C |

## Player — step 4

| | Site | Kind |
|---|---|---|
| [x] 35ceb45 | `transport.rs` `follow_music`, `position` (through `edit_time`) | B |
| [x] 35ceb45 | `transport.rs` `play_stretch`, `after_edit` (through `locate`) | C |
| [x] 35ceb45 | `transport.rs` crossfade volumes, `cut_level`, `change` | D |
| [x] 423db4c | `transport.rs` `carries_on` (`Media::plays`: equal speeds) | F |
| [x] b0cfb88 | `transport.rs` `PREROLL`, sound lead, catch-up: edit seconds at the cut's speed | D 31 |
| [x] stays | `transport.rs` `follow_stretch`, `fps`, `frame_len`, toggle at the end: edit time already | F |
| [x] stays | `keys.rs` `step_frame`, `seek_to`, `aimed`: a step is one clip frame | B C F |
| [x] 35ceb45 | `keys.rs` `stops` | A B |
| [x] 35ceb45 | `effects.rs` `focus`, `effect_focus`, `go_to`, key gap | A B C F |
| [x] 35ceb45 | `marks.rs` `MarkSelect` | B C |
| [x] 35ceb45 | `cuts.rs` `cut_labels` (length shown), Trim / TrimToPlayhead (`frame_len`) | A F |
| [x] 35ceb45 | `transitions_panel.rs` length steps, `join_window`, crossfade shown | D |
| [x] 35ceb45 | `info.rs` «On the edit» | A |
| [x] 35ceb45 | `framing.rs` `grade`, `sphere` (edge and style at clip `t`) | B D |
| [x] 35ceb45 | `bars.rs` `timeline_data`, `can_split` | A F |
| [x] 35ceb45 | through helpers only: `overlay_ui`, `record_ui`, `bin`, `groups`, `bin_player`, `file`, `jobs`, `sound_ui` | — |

## Agents and CLI — step 5

| | Site | Kind |
|---|---|---|
| [x] d81e496 | `apply.rs` `describe` (cut span, markers, crossfade text) | A B D |
| [x] d81e496 | `check.rs` short cuts, pan per second | A B F |
| [x] d81e496 | `preview.rs` tile times, bands, `frame_at` | A B C F |
| [x] 9283654 | `duck.rs` `sources`, `speech` (`Source.speed`) | B |
| [x] d81e496 / 31 | `mixmeasure.rs` `cut_sounds`, windows, items | A E |

`check` and `preview` read media, so the snapshots don't cover them:
each gets a test of its times before it moves.

## Export — step 6 (and card 31)

| | Site | Kind |
|---|---|---|
| [x] 13adabc | `export.rs` `cut_frames` (frames = clip span × fps) | A E |
| [x] 13adabc | `export.rs` `overlaps`, `edit_seconds` | D A |
| [x] ad46d38 | `export.rs` `stretches` (edit seconds; `Sound::reads` the clip span) | E D |
| [x] 13adabc / 31 | `export.rs` `overs`, `camera` (`t = start + k/fps`) | B E |
| [x] 13adabc | `export.rs` handles (clip seconds) | D |
| [x] ad46d38 | 31: `pipeline.rs` `encode`, `player/render.rs` `encode` (`Reading`: frames picked by time) | E |
| [x] ad46d38 | 31: `ffmpeg/mod.rs` `sound_inputs`, `sound_stream`, `Encoder::new` | E |
| [x] ad46d38 | 31: `ffmpeg/mix.rs` `level_filter`, `afade`, `acrossfade`, `pad`, `Sound::of_cut` (tempo before the pad) | D E |

## Media — card 31 only

| | Site | Kind |
|---|---|---|
| [x] b0cfb88 | `playback.rs` `now` (clip time at the speed), `start_audio` (`filter_at`); `step`, `snap` stay clip frames | E F |
| [x] stays | `source.rs` `spawn`, `pump` (pts): clip time, the frames due shown | E |
| [x] b0cfb88 | `audio.rs` `prepare` (the tempo in its filter), `elapsed` (edit seconds heard) | E |
| [ ] 31 | `music.rs` `MusicPlayer::follow` (J/L sound at 1×: a speed is refused on a J- or L-cut) | E |
| [x] b0cfb88 | `media.rs` `Media::for_cut` (its speed), `plays` | E |
| [x] b0cfb88 | `transport.rs` partner priming, `sound_lead`, `p.now() < cut.end` | E |
