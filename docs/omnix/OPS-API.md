# Operations API: one way to change a project

Gap 41, `CORE-API.md` steps 6 and 9, done before 7, 8 and 10 (user,
2026-11-02: "move to the core API for AI and automation, so our changes
can be checked by integration tests"; and: research the right shape
first, so it extends cleanly instead of growing spaghetti). This page is
the design, for the user's approval before any code.

## Why

- **Where it hurts today.** 72 places in `src/player/` change the
  project. About 40 of them write their rule inline in a GUI handler,
  where only clicking can test it. This week's bugs were all of that
  kind: the duck stacking, «Lower» stacking, a selection not let go.
- **Undo is spread out.** Four mechanisms carry it (`checkpoint`,
  `attempt`, `begin_drag` with a `dragging` flag that six messages
  clear, `Edit::delete`). Five handlers take an empty undo step.
- **Written twice.** The CLI changes projects by its own route
  (`loudness::normalize` repeats the GUI's normalize, cuts only).
- **Wanted next.** The AI editor (`AUTO-EDIT.md`) needs a stable,
  described way in, and so do integration tests.

## What others do (research, 2026-11-02)

| Who | How edits are made | What we take |
|---|---|---|
| Kdenlive | Model methods return `bool`: «false = nothing changed»; undo as lambda pairs; ~50 headless Catch2 test files on the timeline model, `checkConsistency()` after every step and undo; a libFuzzer that turns crashes into regression scripts | The «failed op changes nothing» contract; the invariant check; fuzzed op sequences |
| Ardour | `MementoCommand` (before/after state) and `StatefulDiffCommand`; Lua scripts use the GUI's transaction calls; empty transactions dropped | Scripts and GUI share one transaction boundary; no empty steps |
| Olive | Classic command objects with hand-written `undo()` | Avoid: every inverse is a latent undo bug |
| Blender | Operators: an id, typed properties (the schema), `poll`, `execute`; «Adjust Last Operation» undoes and re-runs with new values. Its own docs warn that ops depending on hidden context (selection) are hard to script and fail without saying why | Absolute values, re-run from before (our «from the original» rule); no hidden context |
| OpenTimelineIO | Edits (insert, trim, slice, ripple, roll…) as plain functions on plain data, no undo or UI | The core is pure functions; undo is the host's job |
| Graphite (Rust) | Nested `Message` enums per area, one dispatcher; snapshot undo with `Start/Commit/Cancel` transactions that leave no step if nothing changed; layers by id | Nested enums per area; transactions for drags; ids |
| Zed / GPUI (Rust) | Each action a serde + `JsonSchema` struct, named, with deprecated aliases | Schema for the AI's tools from the same type |

Sources: Kdenlive `src/undohelper.hpp`, `tests/`, `fuzzer/`; Ardour
`memento_command.h`, the Lua scripting manual; Olive
`app/undo/undocommand.h`; Blender `info_gotchas_operators`; OTIO
`algo/editAlgorithm.h`; Graphite `document_message_handler.rs`; gpui
`action.rs`.

## The design

### 1. `Op`: a closed enum, nested per area

```rust
// src/model/ops/mod.rs
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "area", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Cut(CutOp),
    Sound(SoundOp),
    Item(ItemOp),     // overlays and text
    Key(KeyOp),       // view, effect and motion keyframes (with step 8's curve)
    Project(ProjectOp),
    Delete(Target),
}

// src/model/ops/sound.rs
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum SoundOp {
    /// Its own level, dB, absolute (the level tag).
    Level { sound: SoundRef, db: f64 },
    Mute { sound: SoundRef, on: bool },
    Fade { sound: SoundRef, end: End, seconds: f64 },
    /// Points on its own line in `from..to` (file time) replaced by these.
    Line { sound: SoundRef, from: f64, to: f64, points: Vec<VolumeKey> },
    /// The music lowered `depth` dB over these speech stretches; replaces
    /// a duck before (`duck::unduck`).
    Duck { depth: f64, speech: Vec<(f64, f64)> },
    /// Normal level from a measured loudness, evened or not.
    Normalized { sound: SoundRef, lufs: Option<f64>, even: bool },
    LowerForVoice { sound: SoundRef },
    Restore { sound: SoundRef },
    // …
}
```

- **Why an enum, not trait objects.** We own every op. An enum gives
  exhaustive `match` (the compiler finds every place to update), one
  serde and schema derive, and no registry tricks. A trait registry pays
  off only for third-party plugins.
- **Why nested per area.** One flat enum of ~60 variants turns into the
  137-variant `Message`. Nested, each area is its own file:
  `ops/sound.rs` holds `SoundOp` and `fn apply(p, op) -> Result<Outcome>`.
  A new area is a new file and one line in `Op`. This is the extension
  point the user asked about.
- **Ops are model-only.** No playhead, selection, panels or settings.
  The GUI resolves those into explicit values before making the op
  (Blender's lesson; Graphite mixes UI messages in and can't run
  headless).

### 2. Addressing: ids, not indices

An index shifts when something before it is added or removed. Then the
second op of a batch, or an AI's next call, hits the wrong thing. The
selection bug class we just closed comes from this too.

- Cuts, sound clips and overlay/text items get an `id: u32`, unique in
  the project, from a counter in `Project` (deterministic in tests).
- Saved in the JSON as an optional field; a project without them gets
  them on load. Older versions refuse unknown fields, so a project saved
  by the new version won't open in the old one (as with every field we
  add).
- Points and keyframes are addressed by their sound or cut and a time
  (they're already unique by time, `volume_key_at`'s 0.01 s).
- Indices stay inside the code; they never cross the API.

### 3. Values are absolute, and operations start from the original

The user's rule (2026-11-02). `Level { db }`, not «lower by 3». An op
that derives from the sound (duck, normalize, lower) keeps its own layer
and is replaced when applied again: twice equals once. Then a retry by
an agent, a re-run with another value, or Blender's «adjust last op»
can't compound. Relative ops only where people think relatively
(nudge), and the doc comment says so.

### 4. `Editor`: apply, batches, transactions, undo

```rust
pub fn apply(p: &mut Project, op: &Op) -> Result<Outcome, OpError>; // pure, no undo

pub struct Editor { project: Project, undo: Vec<Step>, redo: Vec<Step>, open: Option<Txn> }
struct Step { label: String, before: Project, ops: Vec<Op> }

impl Editor {
    pub fn apply(&mut self, op: Op) -> Result<Outcome, OpError>;          // one undo step
    pub fn batch(&mut self, label: &str, ops: Vec<Op>) -> Result<Vec<Outcome>, OpError>; // all or nothing
    pub fn preview(&self, ops: &[Op]) -> Result<Diff, OpError>;           // dry run on a copy
    pub fn begin(&mut self, label: &str);  // a drag or a run of typing
    pub fn commit(&mut self);              // no step if nothing changed
    pub fn cancel(&mut self);              // back to before (Esc during a drag)
}
```

- **Undo stays snapshots.** A project is small JSON; a copy per step is
  what we do today, and it needs no inverse ops. If it ever grows,
  `Arc` on the big parts shares what didn't change, without changing the
  API.
- **A failed op changes nothing.** `apply` checks first, and the editor
  restores `before` on any error, so a batch is all-or-nothing.
- **Drags** become `begin` on press, ops on every move, `commit` on
  release. This replaces `begin_drag` and the `dragging` flag, and fixes
  the five empty steps.
- **Each step keeps its ops.** That is a journal: what the AI did, in
  words, and a bug in the player can be replayed as a test.

`Outcome { created, removed, changed: Vec<Ref>, summary: String,
warnings }`: `summary` reads «music clip #7: level −3.0 → −6.0 dB».
`OpError` names the field and the allowed range (`NotFound`,
`OutOfRange`, `Conflict`, `Invalid`), so an AI can correct itself.

### 5. Ops that need measuring: measure, then apply

Duck, normalize and colour tracking need ffmpeg and take seconds. The
core stays pure:

1. **Measure** (media layer, on a thread, reads only): `find_speech`,
   loudness, the tracker.
2. **Apply** an op that carries the result (`Duck { speech }`,
   `Normalized { lufs }`), checked against the project's version (today
   a duck or normalize result lands on whatever the edit has become by
   then).

The CLI and the AI can also pass coarse intents (`duck_music { depth }`)
that a resolver turns into measure + op. Undo, the journal and the tests
see only the plain ops.

### 6. Front ends

- **Player.** A handler turns its `Message` into an `Op` and calls
  `editor.apply`; it keeps only UI state (selection, playhead, panels).
  Moved area by area, each a commit.
- **CLI.** `omnix apply project.json ops.json [--dry-run] [-o out]`:
  one batch, prints each outcome and the diff. `omnix describe
  project.json`: what an op needs (ids, names, times), in short text.
  The existing `duck` and `normalize` become thin wrappers over it.
  `omnix blank DIR` makes the black picture of a blank cut (gap 32)
  and prints its path for a cut `add` op.
- **AI (later, milestone 10).** One tool per op from its JSON Schema
  (`schemars`, doc comments as the descriptions), read-only query tools
  (`describe`, `find_speech`, `mixcheck`), `preview`, `undo`.

### 7. Tests

- **`ProjectBuilder`**: a small project in a few lines, deterministic ids.
- **`check(&Project)`**: invariants after every op, undo and redo:
  ids unique, points sorted, no overlaps on a lane, crossfades within
  limits (Kdenlive's `checkConsistency`).
- **Script tests**: `tests/ops/*.json` (ops in, expected summary or
  project out), run through the same code as `omnix apply`.
- **Integration with sound**: ffmpeg generates a tone, noise and a
  «voice» (an on/off tone) in the test. Apply ops, then assert on
  `mixcheck`'s numbers and the export's length and loudness. No footage.
- **Property tests** over random op sequences on a small project:
  apply then undo equals before; a failed op changes nothing; absolute
  ops twice equal once; ops and projects survive a JSON round trip. A
  failing sequence is shrunk to a minimal script and saved as a
  regression file (Kdenlive's fuzzer route).

Not covered: the look and the mouse (the squeezed slider). Those stay
the user's to check.

## Order of work (each a commit, behaviour unchanged unless it says)

Progress (2026-11-02): **the order of work is done.** What an agent
still lacks is listed in the last row and in «Later» below.

| # | Step | State |
|---|---|---|
| 1a | Ids on cuts, sound clips, items (`Project::settle_ids`, saved in the file) | **Done** `d48af1b` |
| 1b | `src/model/` with `edit.rs`, `selection.rs`; a test that the core imports no player, ffmpeg or iced | **Done** `23aff2d` |
| 1c | Transactions: `Edit::begin`/`commit`/`changing` replace `begin_drag` and the `dragging` flag; no empty steps | **Done** `30b0959` |
| 2a | `Op`, `SoundRef`, `Outcome`, `OpError`, `Edit::apply`/`batch`, `check`; sound ops; `omnix apply`/`describe`; first unit and CLI tests | **Done** `2e22ce9` |
| 2b | The player's sound menu, level tags, fades and duck on ops | **Done** `5955b28` |
| 2c | Normalize as measure + apply, by id (`CORE-API` step 6; Even1; the CLI's `--track`); points as ops (add, K, move, delete, continue; named by time); the duck's speech found again if the edit changed meanwhile | **Done** `0bcb553`, `2d259da`, `5b5dc65`, and the duck's check |
| 3 | Cuts (`CutOp`: add, trim, move, split, crossfade, stabilisation, LUT, delete) and sound clips (`ClipOp`: add with the SFX 2 rule, move, trim, split, other SFX lane, delete); the edit's cut rules moved from `Edit` to `Project`; `Outcome::made`; the player's handlers and `Edit::delete` on them; an export test driven by an ops script | **Done** `bb87844`, `755603c`, `c7d38cd`, `9dfba52`, and the test |
| 4 | Overlays and text (`ItemOp`: add pictures, add text, move, trim, set text whole, delete); the player's handlers and Del on them | **Done**, the two commits after `79f53bd` |
| 5 | Keyframes: `model::curve` (`Key`, `key_at`; CORE-API step 8), `KeyOp` for effect, motion and view keys (view keys get where the stick and tracks point as data, `Aims`); the effects panel, K, Auto keyframe, diamond and mark drags, colour-track keys on them; `Edit::checkpoint` gone from the code | **Done** `36c216f`, `cc2eb5a`, and the two after |
| 6 | `proptest` on random op sequences (found a trim panic and inexact floats in JSON, both fixed); `omnix ops-schema` (`schemars`, doc comments as descriptions); `apply` without `-o` is the preview; `describe` the query | **Done** `cd89a11`, `74e2cca`, `e0b5b5c` |
| 7 | Transitions (gap 25, 2026-11-08): `CutOp::Transition` (a join's look, on the cut after it), `TransitionKnob` (its settings), `Edge`/`EdgeKnob` (a look on a cut's start or end), `Template` (a join whole from a saved template, all or nothing); overlap and edge looks refuse each other's op; `describe` says each cut's join, in and out with their settings; `omnix looks` lists every look's key and settings (key, range, default, choices) and the templates' library, `--template NAME` prints one as a `template` op takes it | **Done** |
| — | The edit's fades and the clip-sound and track levels became ops (`SoundOp::EditFade`, `TrackLevel`). Left as a direct write inside a begin/commit: a project's load clean-up | |
| 13 | Audit C4–C7: values held to `grade::EFFECTS` and `Place::RANGES`, a new key to its cut's or item's time, a view to its field of view; `apply` warns of missing files and LUTs and cuts past their clip; `ItemOp::TextField` (one field of a text) | **Done** |
| 12 | The aspect (audit C3): `ProjectOp::Aspect` (`"area": "project"`, "16:9" or none for free); the player keeps it in the edit, so changing it is an undo step and a change to save; `describe` says it | **Done** |
| 8 | Effects and styles (gaps 87, 90, 2026-11-09/10): `KeyOp::EffectValue` / `PlaceValue` (a value for the whole cut or item, or the automation point at the playhead), `KeyOp::Clear` with automation; `CutOp::Style` / `StyleKnob`, `ItemOp::Style` / `StyleKnob` sharing their checks; `ItemOp::Kind` (sticker, polaroid, inset); `omnix looks` lists every effect with its range, the kinds, the ins and outs | **Done** |
| 9 | Automation (gap 39): `AutoOp::Add`, `Move`, `Trim` (outermost points follow the edges), `Delete`, `Point`, `PointDelete`, `PointEase`, `Follow` (a place made from a cut's object track, `CutOp::Track`/`Untrack`), `Animate` (an in or out as clips), `Clear` (a value's clips gone, for a reset) | **Done** |
| 10 | Lanes (gaps 70, 97): items on a track's lane (`AddText`/`AddPictures`/`AddBars` take `lane`, `Move` takes `lane`), `AddLane`, `RemoveLane` (empty, not the last), `LaneName`, `LaneHidden`; an item is addressed by id, its place as (track, lane, index) | **Done** |
| 11 | Measured then applied, beside the player: `omnix autocolour` (gap 89) measures and sets effect values; `omnix normalize`, `omnix duck` as before | **Done** |

What differs from the sketch above, as built: the `Editor` is the
existing `Edit` (in `src/model/edit.rs`) with transactions and `apply`
added, not a new type; the areas are `Cut`, `Clip` (sound clips),
`Item`, `Key` and `Sound`, no `Project` or `Delete` (each area deletes its
own); `Op` is tagged `"area"` and each area's enum `"op"`, so a sound op
in a file reads `{"area": "sound", "op": "level", "sound": {"clip": 4},
"db": -8}`; `Outcome` has a `summary` and the id an op `made`.

## Later: a review of the core (2026-11-02, the user asked after pack 2)

Soon (small each) — **all four done 2026-11-03** (`82464d0` 4,
`19a1bb7` 3, `ae1e7f3` 2, and the selection's commit 1):
1. **The selection by id.** `Selection::Cut(usize)` names an index while
   ops name ids, so every move, delete and undo remaps it by hand
   (`moved_index`, `still_valid`, `after_edit`). By id that class of «the
   wrong thing selected after an edit» goes.
2. **A colour track's key by id.** `follow_track` applies to `done.cut`,
   the index when tracking began (a minute or two before); cuts moved
   meanwhile put the key on another. Duck (version) and normalize (ids)
   are already safe.
3. **A bound on undo.** Each step keeps a copy of the project, with no
   limit; cap it (~300 steps).
4. **`normal_gain` in the core.** `model::ops::sound` reaches
   `crate::loudness`, which runs ffmpeg; the boundary test sees only
   direct imports. Move `normalizing_gain` and `MOST_GAIN` into the core.

Later:
5. **`ProjectOp`** for what's still a direct write: a project's load
   clean-up (the fades, levels and aspect are ops now).
   Needed for the AI and for the lane headers (gap 54).
6. **A journal**: each undo step keeps its ops, so a player session can be
   replayed as a test.
7. **`check()` after every op in debug builds**, so the player catches a
   broken project as the property test does.
8. **The player's per-message `settle_ids`**: ops give ids themselves now.
9. `Outcome` with the ids an op changed, a readable diff of a batch, one
   tool per op for an agent, query tools beyond `describe`.

Kept as they are: the enum per area, ids in the file, transactions, «from
the original»: they held through 60 000 random sequences and packs 1–2.

## Bugs found while taking stock (fixed along the way)

- Empty undo steps: `OverlayKeyMove`, `EffectMsg::KeyMove`,
  `Text(change)` (fixed by 1c), `OtherSfx` (by its op, 3),
  `EffectMsg::Key` (begin/commit, 5a).
- «Show line» twice doubled its points (found by the twice-is-once
  test, fixed in 2a).
- K with a point selected on a cut's line keyed the playing cut's line,
  not that cut's (`keys.rs`; fixed in 2c).
- Duck, normalize and colour-track results applied to the edit as it is
  when they arrive. Duck now finds the speech again, normalize names its
  sounds by id (2c); colour tracking is left for the keyframe step.

## Decided (user, 2026-11-02)

1. **Ids are saved in the project file**: an `"id"` on each cut, sound
   clip and overlay/text item; a project without them gets them on load.
2. **`schemars` and `proptest`** may be added: `proptest` (dev only) with
   the first tests, `schemars` when the AI step comes.
3. **Times stay `f64` seconds**, compared within a tolerance.
