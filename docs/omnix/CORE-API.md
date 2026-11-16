# Core, media, player: what to unify and separate

Gap 41 (user, 2026-11-01): "a check, written into a separate file, of
the possibilities to unify and separate out the core / functionality /
API, which makes further building simpler and divides the architecture
nicely". The second pass after `ARCHITECTURE.md`'s refactor (gap 35),
read at `2b43760` (84 tests). This page was the plan; each step below
became a commit. Worked off by 2026-11-07 but step 7's last moves (gap
41's card); what's kept as it is: `ARCHITECTURE.md`.

Upstream's files stay untouched so they still merge: `view.rs`,
`grade.rs`, `text.rs`, `check.rs` are as upstream; `overlay.rs` only has
additions. Their `grade_at`, `place_at`, `Timed`/`retime` are wrapped,
not edited.

## What is still written more than once

**A. «How loud is this sound at time t» — five places, already apart.**
- A sound clip playing: `player/music.rs` (track level × own level ×
  `envelope` with crossfades).
- A sound clip's waveform on the timeline: `timeline.rs`
  (`draw_sound_lane`) builds a fake `MusicClip` to call `envelope`,
  writes the «off at `OFF_DB`» rule again, and passes no crossfades, so
  an overlap isn't drawn as heard.
- The contact sheet, `preview.rs`: leaves the clip's own level out
  (checked 2026-11-01): **a muted or lowered sound clip draws at full
  level on `preview` sheets**, unlike the export.
- A cut's level twice in `transport.rs` (the crossfade partner and
  `clip_volume`) and again in `timeline.rs`.
- «Silent at `OFF_DB`» written four times (`MusicClip::own_db_at`,
  `OwnSound::db_at`, `timeline.rs`, `cut_gain`); the equal-power quarter
  sine twice (`transport.rs`, `music.rs`); the cut and clip `afade`
  chains in `ffmpeg/mix.rs` nearly the same.

**B. Two families for volume lines** (a track's line and an own line,
handled side by side everywhere). **Resolved 2026-11-01:** steps 2 and 4
made them one family, then the track's line was removed altogether at
the user's ask (gap 33i): every point is on a cut's or a sound clip's own
line (`project::Own`).

**C. Fades, a third copy of the same split:** `Music::set_fade` and
`Cut::set_sound_fade`, `MusicFade` and `SoundFade`, `Drag::Fade` and
`Drag::CutFade`. `OwnSound` already holds both kinds' fades.

**D. «The key at t, or a new one with the value there»** three times
(`volume_key_at`, `effects.rs`'s `change_effects` and `change_place`,
the last two line for line); sampling a keyed value three times
(upstream's `grade_at`, `place_at`; our `volume_at`).

**E. Selection:** five loose `Option`s; «select this and clear the rest»
written by hand about eight times across `tracks.rs`, `cuts.rs`,
`overlay_ui.rs`, `record_ui.rs`, `file.rs`. **Resolved 2026-11-02**
(step 5): one `Selection`; an overlay picked no longer leaves its cut
selected too.

**F. Normalize twice:** the CLI's (`loudness::normalize`, cuts only) and
the dialog's (`sound_ui.rs`); «apply what was measured» in both; Lower
and Restore are edit rules living in the GUI.

**G. Matches on `SoundTrack` outside the enum** (dialog titles in
`tracks.rs`, lane colours in `timeline.rs`), against step 6's rule.

## The layers

| Layer | Holds | Must not import |
|---|---|---|
| **core** (`model/`) | project, overlay data, edit (undo), ops, curve, mixdown (level maths), selection | `ffmpeg`, `player`, `iced` |
| **media** | `ffmpeg/*`, `capture`, loudness measuring, `Waveform::read`, `cache`, `fsutil` | `player` |
| **render** | upstream stitch and colour (CPU), `gpu/` (`gpu.rs`, `render.rs`, shaders) | `player` |
| **player** | iced only | — |
| **cli** | clap → ops, export | — |

Where the lines are crossed today:
- `export.rs` uses `player::render` (the GPU export): `gpu`/`render` to
  `src/gpu/` was planned in `ARCHITECTURE.md` step 1, not done.
- `preview.rs` uses `player::music` (`Waveform`, `gain`, `envelope`).
- `cache.rs` and `lut.rs` use `player::config_dir`: it belongs in
  `fsutil`.
- `player/edit.rs` has no iced in it: model code in the player's folder.
- The model reaches into media: `Cut::sound_file` → `ffmpeg::proxy`
  (touches the disk), `resolve_lut` → `lut::find`.
- `loudness.rs` mixes pure maths (`normalizing_gain`), ffmpeg measuring
  and a printing CLI command.
- Edit rules in GUI handlers: the K-key point logic (`keys.rs`), Lower
  and Restore, applying a normalize result, the delete arms
  (`cuts.rs`), placing a recorded take (`record_ui.rs`).

No separate crates yet: modules, and one test that fails if anything
under `src/model/` imports `player`, `ffmpeg` or `iced`.

## The API for what comes next

Kept small; sketches, not final signatures.

```rust
// model/curve.rs: the base of gap 39's automation lanes, effects,
// motion and volume points
pub trait Key: Copy {
    type V: Copy;
    fn t(&self) -> f64;
    fn value(&self) -> Self::V;
    fn make(t: f64, v: Self::V) -> Self;
    fn ease(&self) -> Ease { Ease::Linear }
}
pub trait Lerp: Copy { fn lerp(a: Self, b: Self, u: f64) -> Self; } // f64, Grade, Place
pub fn sample<K: Key>(keys: &[K], t: f64) -> Option<K::V> where K::V: Lerp;
pub fn key_at<K: Key>(keys: &mut Vec<K>, t: f64, tol: f64) -> usize where K::V: Lerp;
pub fn drag<K: Key>(keys: &mut [K], i: usize, t: f64, v: K::V, range: (f64, f64));
```

```rust
// model/params.rs: one address for whatever the UI or an agent automates
pub enum Param { Volume(Own), Effect(Target, usize), Place(ItemRef, usize), Speed(usize) }
```

```rust
// model/mixdown.rs: the player, the timeline and the contact sheet call
// these; the export's graph is tested against them
pub fn equal_power(x: f64) -> f64;
pub fn cut_gain(p: &Project, i: usize, clip_t: f64) -> f64;
pub fn clip_gain(track: &Music, i: usize, t: f64, video_end: f64) -> f64;
```

```rust
// model/selection.rs (built in player/ beside Edit, step 5)
pub enum Selection { None, Cut(usize), Clip(SoundTrack, usize), Point(Own, usize), Item(ItemRef) }
impl Selection {
    pub fn still_valid(self, edit: &Edit) -> Self;
}
impl Edit { pub fn delete(&mut self, s: Selection) -> bool; } // the delete arms in one
```

```rust
// model/ops.rs: for the CLI and the AI editor (`AUTO-EDIT.md`)
pub enum Op { AddPoint(Own, f64, f64), MovePoint(Own, usize, f64, f64), Fade(Own, bool, f64),
              Trim { .. }, Move { .. }, Split { .. }, SetSound(Own, SoundChange), Delete(Selection), .. }
impl Edit { pub fn apply(&mut self, op: Op) -> Result<Outcome>; } // takes its own undo step
```

The GUI's `on_*` handlers then turn a `Message` into an `Op` and keep
only UI state. Timeline lanes (`ARCHITECTURE.md` step 8) behind one
trait, so a press maps to a drag once for every lane:

```rust
trait LaneView {
    fn height(&self, rows: &Rows) -> f32;
    fn draw(&self, tl: &Timeline, f: &mut Frame, b: Rectangle);
    fn hit(&self, tl: &Timeline, p: Point, b: Rectangle) -> Option<Hit>;
}
enum Hit { Block(usize), Edge(usize, bool), Fade(Own, bool), Point(Own, usize), Line(Own) }
```

Before speed ramps (gap 31): `offsets`, `len()`, `locate`, `edit_time`
and the mix's trims all assume edit time = clip time. One
`Cut::clip_time(local)` first, used by all of them.

## Order of work

Steps 6 and 9 come first, as the operations API (`OPS-API.md`,
2026-11-02: researched, its own order of work); 7, 8 and 10 after.

One commit each, behaviour unchanged unless the step says so. Tests
(`TESTING.md`) come first, so each step lands on a net.

| # | Step | Size | Risk |
|---|---|---|---|
| 1 | **Done 2026-11-01** (`73cb3e8`; fixes `0f04821` timeline crossfades, `1898a41` contact sheet): `src/mixdown.rs` (`equal_power`, `own_level`, `cut_gain`, `envelope`, `clip_gain`), every level site calls it | S | low |
| 2 | **Done 2026-11-01** (`11c698b`): one line per sound (since folded into `Own`/`OwnSound`, `own_sound`) | S | low |
| 3 | **Done 2026-11-01** (`7a298f0`): `OwnSound::set_fade`, one `Message::Fade`, one `Drag::Fade` | S | low |
| 4 | **Done 2026-11-01** (`fe8d675`), checked by the user (2026-11-01); then the track's line removed altogether at their ask (gap 33i, `5aeba4c`): points are only ever on an own line, `PointAdd/Select/Move(Own, …)`, `Drag::Point`, one `selected_point`, one Del, one K | M | medium: drags, Ctrl+click, K |
| 5 | **Done 2026-11-02** (`c1ee19d`), now `model/selection.rs`: `Selection`, `still_valid`, `Edit::delete`; four `App` fields became one. No `after_move`: a moved cut is selected outright | M | medium: ten files; removes a known source of bugs |
| 6 | **Done 2026-11-02** (as OPS-API step 2c): normalize measured then applied by `SoundOp::Normalized`; lower and restore are ops; the CLI gains `--track` | S | low |
| 7 | Moves: `edit.rs` → `model/` (**done**, `6c3bf9c`), `config_dir` → `fsutil` (**done** 2026-11-05, `fsutil::app_dir`), `Waveform` → media, `Cut::sound_file` → media (open); `gpu`/`render`/shaders → `src/gpu/` looked at and kept (`ARCHITECTURE.md`) | M | low logic risk |
| 8 | **Done 2026-11-02** (`80ffdc1`, as OPS-API step 5): `model::curve` (`Key`, `key_at`, `find`) for effect and motion keys; volume points stay `project::volume_key_at` (no ease, dB lines) until the automation lanes | M | low |
| 9 | **Done 2026-11-02**: `OPS-API.md`, all its steps | M | medium |
| 10 | **Done 2026-11-07** as the timeline package (the audit's last item): one `Hit` per canvas, shared drawing helpers, one `Picked`, blocks holding the cut, clip or item they show, `Marks::update` split | L | high: drawing and the mouse everywhere; last, after 4 and 5 |

## Small things found

- Fixed since: the orphan doc comments, the thin wrappers
  (`cut_wave_file`, `music::gain`), `ARCHITECTURE.md`'s old sizes.
- `#[allow(clippy::too_many_arguments)]` on `draw_own_line`: it wants a
  small struct.
- `Message` has grown to about 195 variants and `handle` names each one;
  nested enums per area (as `Sound`, `Record`, `Effect`, `Template`
  already are) would make the dispatch one line per area.
- No over-engineering found. The risk is the opposite: the same thing
  written again.
- `OwnSound` (seven `&mut` fields) looks heavy but is right: the JSON
  names differ (`sound_fade_in` vs `fade_in`) and serde's `flatten` can't
  be combined with `deny_unknown_fields`.
