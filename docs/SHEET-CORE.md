# The contact sheet's core: one way to read and change `notes.json`

Designed and approved 2026-11-04 and built the same day (steps 1-8
below), as Omnix's `OPS-API.md` was for projects. The reference for the
core: the format, the model, the ops, the store, the CLI. The user: start the move to Rust with the
contact sheet's core; "a core API for the contact sheet too, so changes
are easier in both places at once". `notes.json` is footage-agent's name
for the file; in a Dailix project it is `dailix/sheet.json`, the same
format (`Layout::sheet`).

## Why a core, and why first

- **Three writers, no shared rules.** The web sheet saves the whole file
  from the browser's memory; Omnix's `notes.rs` patches it by hand
  (segments, roles, notes, delete); footage-agent's Python reads it in
  four places (`query.py` twice, `todo.py`, `export/omnix.py`). Each
  knows the format its own way. Two writers already needed a guard
  against stale saves.
- **No ids.** A mark is a list item. Omnix addresses one by its place
  and checks it's unchanged (`t`, `end`) before writing; the AI (the
  brainstorm in footage-agent's `CONTACT-SHEET-NEXT.md`, D3) and a cut's
  link back to its mark need a stable id.
- **What comes next stands on it:** the studio's own window (every edit
  through the core), Omnix (the same calls), the AI (operations, a
  schema, a text view), tests (operations replayed, the file checked).

## The format today (read from the code and the Kos file, 2026-11-04)

```text
{
  "days":  { "<YYYY-MM-DD>": { "note": "<the day's story>" } },
  "clips": { "<day>/<file>": {
      "rating": 2 | 1 | 0 | null,   // ✓ take, ? think, ✕ junk, unrated
      "note":   "<about the clip>",
      "audio":  "dnn6",             // the cleaned voice chosen (voice/<day>/<clip>/<name>.flac); absent: the clip's own
      "marks": [
        { "t": 21.8, "end": 25.0, "role": "broll", "note": "…" },   // a segment
        { "t": 7.0, "note": "…" },                                  // a point (first version's)
        { "t": 26, "kind": "flag", "role": "cut", "note": "…" }     // a flag: a note to self, never material
      ] } },
  "roles": [ { "key": "x_…", "title": "…", "en": "…", "hint": "…" } ],  // the user's own, beside the eight built in
  "ui":    { "shut": ["<day>", …] }   // the sheet's view state: days folded
}
```

Facts that shape the design:
- The sheet writes it with Python's `json.dumps(indent=2,
  ensure_ascii=False)`, no trailing newline; the Kos file is exactly that
  form. Times are rounded to a tenth in the browser, so 27 of them are
  integers (`26`, not `26.0`).
- A segment shorter than 0.4 s is saved as a point (the sheet's rule).
- A rated ✓ clip with no segments goes into a stringout whole; flags never
  count as marking (`query.py`).
- The voice clean-up state lives on disk (`voice/…/*.flac`), not here;
  only the choice (`audio`) does.
- Fields this design doesn't know must survive any write: the web sheet
  keeps adding them (`audio` came that way).

## The design

### 1. The model: the file as read, a typed view over it

Built in step 1 (2026-11-04). First drafted as typed structs with the
unknown fields kept beside them; dropped before code, because the sheet
writes a mark's keys in varying order (`{t, note, end, role}` from a new
mark, `{t, end, role, note}` elsewhere) and typed structs write one
order, so a file would change for nothing.

- `Doc`: the file as read (`serde_json::Value`, keys in their order,
  numbers in their form: `26` stays `26`, `9.0` stays `9.0`, fields this
  crate doesn't know kept at every level). Operations (step 3) change it
  in place; `text()` writes it in the sheet's form (Python's
  `json.dumps(indent=2, ensure_ascii=False)`, no newline at the end).
- `Sheet`: the typed view (`days`, `clips` keyed `<day>/<file>`,
  `roles`), read from the `Doc` for programs to show and for `check`.
  A `Mark`'s kind is derived: `kind: "flag"` a flag, `end` a segment,
  else a point.
- Tests: a made-up sheet with every easy-to-lose thing, written by
  Python's own call, read and written back byte for byte; the same by
  hand on a scratch copy of Kos's sheet (an ignored test given the path
  in `DAILIX_SHEET`): identical.

### 2. Ids for marks

A new optional field `id` on each mark: short, unique in the file
(`m1`, `m2`…), given on load to marks without one and kept forever after.
The web sheet keeps fields it doesn't know on the objects it holds, so
ids survive its saves; a mark it creates comes without one and gets one
at the core's next load. Python readers use `.get`, so they don't mind.

### 3. Operations: a closed enum, as Omnix's

```rust
#[serde(tag = "area", rename_all = "snake_case", deny_unknown_fields)]
pub enum SheetOp { Mark(MarkOp), Clip(ClipOp), Day(DayOp), Role(RoleOp) }

pub enum MarkOp {
    Add    { clip: ClipKey, t: f64, end: Option<f64>, flag: bool, role: Option<String>, note: String },
    Move   { mark: MarkId, t: f64, end: Option<f64> },
    Role   { mark: MarkId, role: Option<String> },
    Note   { mark: MarkId, note: String },
    Remove { mark: MarkId },
}
pub enum ClipOp { Rate { clip: ClipKey, rating: Option<Rating> }, Note { clip: ClipKey, note: String },
                  Voice { clip: ClipKey, audio: Option<String> } }
pub enum DayOp  { Note { day: String, note: String } }
pub enum RoleOp { Add { title: String, en: String, hint: String } }   // key made as the sheet makes it
```

- Absolute values (a role, a note, a span), never "change by": an op
  replayed gives the same file (Omnix's «from the original» rule).
- Addressed by id and key, no hidden context (no "the picked mark").
- `apply` either does the whole op or nothing, and says what it made
  (`Outcome { made: Option<MarkId> }`) or why not (`OpError`: not found,
  invalid, and the check's message).
- The sheet's `ui` has no ops: it is the view's, not the marking.

### 4. `check`: what a sheet must be after every op

- every mark: `t ≥ 0`; a segment's `end > t + 0.4`; a flag has no end;
- ids unique; a role is one of the eight or the sheet's own;
- a rating is 0, 1, 2 or none; a day key is a date; a clip key is
  `<day>/<file>` with that day;
- (with a clip's length known, from the sidecar) marks inside the clip.

### 5. The file (and undo, later)

- Undo: not yet. A snapshot undo (`Edit`, built 2026-11-04) was dropped
  unused (user, 2026-11-05): restoring a snapshot would overwrite what
  another writer saved since. Undo comes as inverse ops through `Store`,
  with a key.
- `Store`: the file on disk. Every write reads the file afresh, applies
  the ops given (id-addressed, so they land on whatever is there now),
  checks, writes atomically with the last version beside it (`.bak`).
  While the web sheet lives it can still save the whole file; its
  server refuses a stale save (footage-agent `3a67dba`).

### 6. CLI, schema, a text view

The studio's binary, beside its window:
- `dailix sheet apply <notes.json> <ops.json>`: the ops, checked,
  written; one line per outcome;
- `dailix sheet describe <notes.json> [--day D]`: a line per mark
  (day, clip, span, role, note) and per rated clip: what the AI reads in
  place of the JSON (brainstorm D12);
- `dailix sheet schema`: the JSON Schema of the file and of the
  ops (`schemars`, as Omnix's `ops-schema`).

### 7. Where it lives

`crates/sheet` in Omnix's repo (the core, a library), a member of its
workspace; Omnix's `notes.rs` keeps only Omnix's side (where a
project's sheet is, how a clip is keyed). Dailix, in its own repo, takes
both crates from Omnix's. It was Dailix's own crate until 2026-11-14,
when the two were made to build each from one clone for publishing
(gap 109): the crate moved with its history.

## The plan, a local commit per step

1. **Workspace and model** — done 2026-11-04 (`0e540b5`). `crates/sheet` with the typed model, the
   unknown kept, `Time`'s integers, load and save. Tests: a synthetic
   sheet read and written byte for byte; every shape of mark; unknown
   fields at each level kept. A one-off check on a scratch copy of Kos's
   file (never committed): identical after a round trip.
2. **Ids** — done 2026-11-04. `Doc::settle_ids`: `m<n>` past the highest,
   in the file's order (the same file, the same ids), the later of two
   sharing one renumbered, the key last in the mark so nothing else
   moves. An id counts only once written: a program settles and saves
   before addressing a mark (the store, step 5). The web sheet only
   pushes new marks and splices deleted ones, never rebuilds one, so it
   keeps the field (read in `report.html`). On a scratch copy of Kos's
   sheet: 86 of 86 marks given ids, nothing else changed.
3. **Operations and `check`** — done 2026-11-04 (`ops.rs`). Mark add /
   move / role / note / remove, clip rate / note / voice, a day's note, a
   role added (keyed as the web sheet keys one, checked against
   JavaScript itself). New things in the web sheet's shapes. `check`
   lists problems (a segment under 0.2 s, Omnix's shortest cut; a flag
   with an end; a twice-used id; an unknown role; keys not
   `<day>/<file>`); an op is refused only for a problem it would add, so
   one already in a file blocks nothing. Kos's sheet (a scratch copy):
   no problems.
4. **Properties** — done 2026-11-04 (`tests/props.rs`): random runs of
   up to 40 ops (bad roles, ratings and ids among them) keep `check`
   empty, read back from their own text, and a refused op changes
   nothing. (The snapshot undo built here was dropped, §5.)
5. **`Store`** — done 2026-11-04 (`store.rs`). No copy held between
   writes: each reads the file afresh, gives ids to marks another writer
   added, applies the ops (all or none), writes whole (beside, synced,
   renamed over) after copying the last version to `notes.json.bak`.
   `read` writes first if it gave ids. Tests: ids written on read; an op
   by id lands on its mark though another writer saved the whole file
   with a new mark before it; a refused op writes nothing and leaves no
   temporary file.
6. **CLI and schema** — done 2026-11-04 (`crates/dailix`). `dailix sheet
   apply <notes> <ops|->` (one op or a list, all or none, a line per
   outcome), `check`, `describe [--day]` (a line per day, rated clip and
   mark: Kos's day reads at a glance), `schema [ops|sheet]`. Tests: the
   text view, the schemas, and the binary end to end (ops applied,
   described, a refused run writing nothing).
7. **Omnix on the core** — done 2026-11-04 (Omnix `587229d`). Its
   `notes.rs` keeps only Omnix's side (where a project's sheet is, a clip
   file's key) and goes through `Store` and ops by id; 176 + 10 tests
   pass. Omnix depends on `../dailix/crates/sheet` by path.
8. **Docs** — done: this page, both contexts, the roadmap.

The studio's window came next, on this core, and is built (`docs/CONTEXT.md`).

## Decided (user, 2026-11-04)

1. **The name:** Dailix; the binary `dailix`, the core `dailix-sheet`
   (`crates/sheet`).
2. **Ids in the real sheet:** yes, written.
3. **Time precision:** what the core is given, to a thousandth; the web
   sheet's tenths stay as they are.
