# Audit of 2026-11-10 — the list to work off

Five read-only reviews (dead and stale code; bugs in the week's work;
design against `code-rules`; the core API's gaps for an agent; Dailix's
voices and sheet), merged and ranked. Each fix is its own commit, and
its line leaves this file when it's done (the history is in git). Line numbers as of `f13a5e6`.

## A. Bugs

All fifteen fixed, each with a test (`git log --grep 'audit A'`, Omnix
and Dailix).

## B. Design (`code-rules`)

B1 (the grade's values by name, one `EFFECTS` table) and B2 (tracks and
items one way each) done.

| # | What | Fix |
|---|---|---|
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

C1–C7 done: `describe` says a cut's view, settings and sound, the
edit's own, a text's look, an item's own time and keys; the aspect is
an op; ops hold values to their ranges and `apply` warns of what's
missing; `looks` lists LUTs and fonts; `text_field` sets one field;
`looks --save-template`; `--cuts` takes ids (C8); the schema gives every op a description and
the values their ranges and defaults (C9).

| # | What | Fix |
|---|---|---|
| C10 | Measures the GUI has, the CLI hasn't: scopes, a graded frame (`preview` draws no grade), the mix's loudness over time | `omnix scopes`, a graded `frame`, `mixcheck --every` |
| C11 | `normalize`/`duck`/`autocolour` can't print the ops they'd apply | `--ops` |
| C12 | Dailix: a trip's description and tags can't be changed by command after it's made; no `dailix voice use` shortcut; a marking naming a voice that's gone isn't flagged | `dailix about`, `voice use`, a `sheet check` warning |

## D. Stale comments and leftovers

All fixed (Omnix `b17d380`, Dailix `9ad0500`). No large dead code was
found; every message variant is built and matched; settings are each
read and written.
