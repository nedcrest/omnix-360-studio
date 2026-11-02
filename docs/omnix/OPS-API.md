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
- **CLI.** `rosmo360 apply project.json ops.json [--dry-run] [-o out]`:
  one batch, prints each outcome and the diff. `rosmo360 describe
  project.json`: what an op needs (ids, names, times), in short text.
  The existing `duck` and `normalize` become thin wrappers over it.
- **AI (later, milestone 10).** One tool per op from its JSON Schema
  (`schemars`, doc comments as the descriptions), read-only query tools
  (`describe`, `find_speech`, `mixcheck`), `preview`, `undo`.

### 7. Tests

- **`ProjectBuilder`**: a small project in a few lines, deterministic ids.
- **`check(&Project)`**: invariants after every op, undo and redo:
  ids unique, points sorted, no overlaps on a lane, crossfades within
  limits (Kdenlive's `checkConsistency`).
- **Script tests**: `tests/ops/*.json` (ops in, expected summary or
  project out), run through the same code as `rosmo360 apply`.
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

1. `src/model/`: `Editor` with transactions, `Op` skeleton, `Outcome`,
   `OpError`, `check`; ids on cuts, clips and items. The player's `Edit`
   becomes `Editor`; drags move to `begin`/`commit`.
2. Sound ops (the area tested most by hand this week): level, mute,
   fades, low cut, clean, line, delete line, duck, normalize as
   measure + apply (step 6), lower/restore. The player's sound handlers
   on them. `rosmo360 apply`, `describe`. First script and integration
   tests.
3. Cuts and timeline: add, trim, move, split, crossfade, delete, LUT,
   stabilisation.
4. Overlays and text.
5. Keyframes, with step 8's curve (`change_effects`/`change_place`).
6. AI: schemas, query tools, preview.

## Bugs found while taking stock (fixed along the way)

- Empty undo steps: `OverlayKeyMove`, `EffectMsg::KeyMove`,
  `Text(change)`, `EffectMsg::Key`, `OtherSfx`.
- K with a point selected on a cut's line keys the playing cut's line,
  not that cut's (`keys.rs`).
- Duck, normalize and colour-track results apply to the edit as it is
  when they arrive, without checking it's still the same.

## Open questions for the user

1. **Ids in the project file**: an `"id"` on each cut, clip and item.
   The alternative (ids only while the program runs) keeps the JSON
   clean, but then `ops.json` for the CLI can't name things stably.
2. **New dependencies**: `schemars` (schemas for the AI, step 6 of the
   order) and `proptest` (property tests, dev only). Both pure Rust, no
   wgpu.
3. **Times** stay seconds as `f64`, compared within a tolerance. OTIO
   uses rational time, but switching would touch everything for little
   gain here.
