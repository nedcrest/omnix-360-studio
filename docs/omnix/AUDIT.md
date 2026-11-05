# Code audit, 2026-11-05: what's left to clean

Five read-only passes over Omnix and Dailix (dead code, code that does
nothing, duplicates, over-engineering, tangled logic, stale docs), each
finding checked before it's fixed. A finding leaves this page when its
commit lands; the page goes when the list is empty. Strictly dead code
turned out rare (Dailix's sheet core had some, gone in `1573719`); the
cost is duplication and drift.

## Bugs found on the way (first)


## Duplicates

Across the two repos:

In Omnix:
- [ ] Info read and drawn twice (bin side pane, Info panel).
- [ ] `Sound` spelt out field by field in `cli.rs` (`render`).
- [ ] Timeline: press and hover each walk what's under the pointer (now in
      the same order, `Marks` and `OverlayLane`): one `Hit` per canvas,
      used by both (CORE-API step 10).
- [ ] Timeline per-lane code (edges, fades, blocks, folded strips,
      crossfade X, lane boxes) copied across lanes; view structs copy the
      model field by field; five selection fields rebuilt from one.

## Tangled

- [ ] `App` mirrors ~33 settings by hand (in at `new`, out at `settings()`).
- [ ] `Marks::update` 241 lines; `export()` 159; `cli::run` 157 (preview
      and export logic inline); Dailix `update` 243.
- [ ] Library modules reach into `player` (`Waveform`, `render`).
- [ ] Tests in the wrong module (`sound_ui.rs` tests cuts and markers).

## Decided by the user (2026-11-05): done

Dailix's snapshot undo dropped (undo later as inverse ops through
`Store`); one loudness meter (ours); upstream's geometry helpers merged;
`check` takes only its options.

## Order (user, 2026-11-05)

This refactor first, all of it; then the rename to omnix (gap 76) with
Omnix's own face (gap 79: the footer «Dmytro Halushko with Claude» and
upstream credited apart; our own start picture and icon, upstream's logo
gone). The rename is cheaper now than later. For gap 79, look for every
place upstream's name or credit shows (footer, About, window title,
desktop entries, packaging, README), not only the footer.

After both: the user checks the cleanup's visible changes with a short
"how to" for each (user, 2026-11-05: never saved a frame; the LUT and the
frame need steps spelled out).


