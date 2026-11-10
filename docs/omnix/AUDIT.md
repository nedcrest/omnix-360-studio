# Audit of 2026-11-10 — the list to work off

Five read-only reviews (dead and stale code; bugs in the week's work;
design against `code-rules`; the core API's gaps for an agent; Dailix's
voices and sheet), merged and ranked. Nothing here is fixed yet; each
fix is its own commit, and the line leaves this file when it's done
(the history is in git). Line numbers as of `f13a5e6`.

## A. Bugs

| # | What | Where | Fix |
|---|---|---|---|
| A1 | **Panic**: trimming an automation clip that hangs past its owner's edge (after the cut or item was trimmed) — `clamp` with min > max | `player/timeline/auto.rs:280` (edge drag), `model/ops/auto.rs:152` (`AutoOp::Trim`) | Guard the clamp; better, clip a cut's or item's automation to its new range when it's trimmed. A click on such an edge also trims it silently |
| A2 | A hidden lane's items still get handles in the viewer: invisible, yet grabbed and dragged | `player/overlay_ui.rs:85-97` | One `Project::shown_lane_refs()` for draws and handles |
| A3 | A fresh project has no lanes, the timeline shows one: its eye and rename fail («lane 0 … not found»), the first «+» changes nothing on screen | `model/ops/items.rs` `LaneName`/`LaneHidden`/`AddLane`, `App::shown_lanes` | The model keeps one lane per track always, or those ops make lane 0 first |
| A4 | An item dragged onto another lane lands where its own lane's neighbours stopped it, not under the pointer | `player/timeline.rs:1786-1791, 1822-1826` | Keep the pointer's time in the drag; no `OverlayMove` while over another lane |
| A5 | Removing a lane (or moving an item off one) leaves `active_lanes` and `extra_lanes` pointing at the wrong lane | `player/overlay_ui.rs:240` | Shift the indices above it, drop those equal |
| A6 | `add_text`/`add_pictures`/`add_bars` with `lane: 7` make seven lanes; a huge one runs out of memory | `model/edit.rs:237` `lane_mut` | Refuse `lane > len` (`len`: new on top) |
| A7 | Dailix import carries `08_graphics` as it is: an imported project gets both folders; a footage-agent project made after the rename loses its `08_pictures` | `dailix/crates/dailix/src/import.rs:45` | Carry both into `PICTURES`, by the layout's constants; a test |
| A8 | Dailix: a voice being made is forgotten when the pane changes clip — making it again races the same `.part` and `_recipes.json`; the error shows on another clip | `dailix/.../pane.rs:129`, `pane_voice.rs:114` | Makes in progress on `App`, keyed by (clip, recipe); the clip in the error |
| A9 | Dailix: `_recipes.json` read-modify-write and a fixed temp name: two makes lose an entry or fail | `dailix/.../voice.rs:97-99`, `sheet/src/voice.rs:174` | `store::replace`-style write; one core `voice::record` re-reading just before writing |
| A10 | Dailix: a voice whose measuring failed has no entry: listed, never measured, gain 0 in the pane and in Omnix | `dailix/.../voice.rs:94-99` | Always write the entry (`lufs: None`); `measure` covers variants without one |
| A11 | Dailix: «Original» picked to compare is undone when a make or use finishes; the sound restarts on every refresh | `dailix/.../pane_voice.rs:95` | A «picked» flag; `hear` only when file or gain changed |
| A12 | `OverlayToLane` skips `ScrubDone`'s clean-up: the snap line stays | `player/overlay_ui.rs:243` | Clear it as `ScrubDone` does |
| A13 | Scopes freeze for good if a map fails; reopened, they show a stale measure | `player/gpu.rs:675` | Back to idle on error; clear on reopen |
| A14 | A late auto-colour result joins an open drag's undo step, or lands on another project opened meanwhile | `player/effects.rs` `AutoColoured` | A project generation check; commit an open change first |
| A15 | Smaller: the `.part` FLAC left in the voice folder on failure; the scratch folder ignores Dailix's TMP place; `mask::shift`/`squared_1d` unguarded for edge sizes; scopes assume 8-bit 4-byte pixels; a range ending at the edit's end may not return; `Playback::hear` and `has_audio` after a bad voice | various | One by one |

## B. Design (`code-rules`)

| # | What | Fix |
|---|---|---|
| B1 | The 18 grade values indexed by raw numbers (9, 16, `SHAPE + 1`…) in ~8 places; the group's reset list and its sliders written apart | Named consts for all 18 in `grade.rs`; one `EFFECTS` table (name, label, hint, range, neutral); a group's sliders from `Group::params` |
| B2 | Track → lanes `match` written 7 times; item-by-id mutable 5 times; `[Track::Overlay, Track::Text]` and the names 6 times | `Project::track_lanes_mut`, `Project::item_mut(id)`; `Track::ALL`, `Track::name()` |
| B3 | The timeline restates the lane model (`lane_meta` tuples); «active lane, clamped» twice; a dead `.max(1)` twice | Hold a `LaneHead` from `Lane`; one `active_lane`; drop the dead clamps |
| B4 | Where a copied-in picture goes, three ways; «inside the project» checked twice | Everything through `folder_for`; one `App::inside_project` |
| B5 | Derived values written out: a music clip's own time ×7, the levels' span ×3 (auto colour must match `Grade::apply`), `luma` ×4, `1.0 / fps()` ×11 | Methods: `MusicClip::local`, `Grade::level`, one `luma`, `App::frame_len` |
| B6 | The film's fades drawn by `effects_view::whole_edit` for the Transitions panel, on `EffectMsg` | Move them into `transitions_panel.rs` |
| B7 | Rules in UI code without tests: the sound menu's entries, `Message::Range`'s clamp, Dailix's `recipe_to_make`, the pane's speech (a copy of `voice::speech` that swallows errors) | Pure functions with tests in the core |
| B8 | `cut.flat` written directly by the player and kept in the project (undo can bring back a stale one); `Edit` derefs mutably, so nothing stops direct writes | `flat` out of the project; drop `DerefMut` and let the compiler list the rest |
| B9 | Long functions: 29 in `player/` over 80 lines; four grew past it this week (`key` 209, `effect_message` 200, `on_overlay_ui` 145, `sound_menu_layer` 109) | Split those four first, by area |
| B10 | CPU/GPU tests miss most of the grade (highlights, shadows, contrast, temperature, tint, vibrance on colour) and the overlay's blur, reveal and letter order beyond the two cases; `grade.wgsl` hard-codes the knee | One test with every value on a coloured picture; name the knee |
| B11 | `from_sheet` puts a whole file's loudness into `cut.loudness`, which means «what the cut plays» | Measure the segment, or say what it is |
| B12 | Two loudness rules: Dailix compares at −20 LUFS (lift ≤ 15), Omnix plays at −16 (≤ 24) | Decide in one place, or say why they differ |

## C. Core API gaps (what an agent can't see or do)

| # | What | Fix |
|---|---|---|
| C1 | `describe` hides a cut's view keys, stabilisation, LUT, note, audio file, «flat» | Lines for each in `look_lines` |
| C2 | `describe` hides most sound: volume points, fades, J/L sound, even/low cut/clean, loudness, duck spans, track and clip-sound levels, the edit's fades | A header line, a line per track, a `sound:` and `points:` line per cut and clip |
| C3 | The project's aspect is GUI state, not an op, not described | `ProjectOp::Aspect`; in `describe` |
| C4 | Ops take values out of range (exposure 1000, view at 500 s, place x 99, trims past the file, missing files and LUTs); `check` doesn't catch them | Range checks in the ops; key invariants in `check`; `apply` warns of missing inputs |
| C5 | No list of LUTs or fonts; a text's style isn't described; `set_text` needs the whole text | `looks` lists them; `describe` the style |
| C6 | Items: `describe` drops place keys, effect keys, source range | Lines as cuts have |
| C7 | Saving a transition template is GUI-only | `omnix looks --save-template` |
| C8 | `--cuts` on `normalize`/`autocolour` numbers by position; ops and `describe` by id | Ids |
| C9 | The schema hides defaults that aren't 0 (opacity, white_in, reveal…) and every range; some ops undocumented; «out of range» prints `f64::MAX` | `schemars` defaults and ranges; doc comments; «∞» |
| C10 | Measures the GUI has, the CLI hasn't: scopes, a graded frame (`preview` draws no grade), the mix's loudness over time | `omnix scopes`, a graded `frame`, `mixcheck --every` |
| C11 | `normalize`/`duck`/`autocolour` can't print the ops they'd apply | `--ops` |
| C12 | Dailix: a trip's description and tags can't be changed by command after it's made; no `dailix voice use` shortcut; a marking naming a voice that's gone isn't flagged | `dailix about`, `voice use`, a `sheet check` warning |

## D. Stale comments and leftovers

Omnix: `effects.rs:574` (`text_controls` says «the text panel»), `:786`,
`:727` (a stray line on `style_of`), `:56` (`EffectMsg::Style` «the cut's»),
the `PARAMS` doc («the first eleven»); `overlay_ui.rs:2`; `bars.rs:238`
(«Text at the left»); `mod.rs:550` (`DeleteCut` deletes far more);
`transitions_panel.rs:197` («a join» — any stretch); `selection.rs:51`;
`cuts.rs:100` («graphics folder»); `bin.rs:68` and its test (voices at
the project's root, made by footage-agent); `docs/omnix/CONTEXT.md:132`
(`08_graphics`). Dailix: `Made::is_current` used only by tests (wire it
in to flag a stale voice, or remove it); `Settings::tmp` used by
nothing yet (say so).

No large dead code was found; every message variant is built and
matched; settings are each read and written.
