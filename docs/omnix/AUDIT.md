# Audit of 2026-11-10 — the list to work off

Five read-only reviews (dead and stale code; bugs in the week's work;
design against `code-rules`; the core API's gaps for an agent; Dailix's
voices and sheet), merged and ranked. Each fix is its own commit, and
its line leaves this file when it's done (the history is in git). Line numbers as of `f13a5e6`.

## A. Bugs

All fifteen fixed, each with a test (`git log --grep 'audit A'`, Omnix
and Dailix).

## B. Design (`code-rules`)

All done (B1–B12). The grade's values by name in one `EFFECTS` table;
tracks and items one way each; the timeline's `LaneHead` and one
`active_lane`; pictures copied through `folder_for`, the project's
folder asked once; `grade::luma`, `grade::level`, `MusicClip::local`,
`App::frame_len`; the film's fades in the Transitions panel's own code;
the range, the sound menu and Dailix's Make recipe and speech as tested
functions; no `DerefMut` on `Edit` outside tests (ops through `put`,
what the player learns of a clip through `learn`; `flat` stays a
`serde(skip)` field the core's view keys read); the four long functions
of that week split (older ones are split as they're next touched); the
GPU against the CPU with every grade value, the knee named. Loudness
(the user's decisions, 2026-11-11): `from_sheet` measures the stretch
each cut plays (B11); Dailix compares voices at −20 and Omnix sets the
film at −16, two purposes, two rules, each saying why (B12).

## C. Core API gaps (what an agent can't see or do)

All done (C1–C12). `describe` says the edit's aspect, levels and fades,
a cut's view, settings and sound, a text's look, an item's own time and
keys; the aspect is an op; ops hold values to their ranges and `apply`
warns of what's missing; `looks` lists LUTs and fonts and saves
templates; `text_field` sets one field of a text; `--cuts` takes ids;
the ops schema describes every op and gives each value its range and
default; `omnix frame`, `omnix scopes` and `mixcheck --every` measure
what the player shows; `--ops` prints a measuring command's ops.
Dailix: `about`, `voice use`, and `sheet check` saying a voice that
isn't made.

## D. Stale comments and leftovers

All fixed (Omnix `b17d380`, Dailix `9ad0500`). No large dead code was
found; every message variant is built and matched; settings are each
read and written.
