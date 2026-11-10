//! The automation lanes' messages (gap 39): what a press or a drag on a
//! lane under the Effects row does, made into `Op::Auto` on the cut it's
//! over. Drags are one undo step each, closed on release (`ScrubDone`).

use super::App;
use crate::model::ops::keys::Graded;
use crate::model::ops::{End, Op, auto::AutoOp};
use crate::view::Ease;

/// What an automation lane's clips belong to: a cut (its index) or an
/// overlay item (its id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoOn {
    Cut(usize),
    Item(crate::project::Id),
}

/// Whose automation lanes: the cuts' (under their Effects row), or the
/// items' of an overlay or text track (under its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneOf {
    Cuts,
    /// A lane of a text or overlay track (gap 70).
    Track(crate::overlay::Track, usize),
}

/// A value a lane moves: a colour effect (`grade::NAMES` index) or, on an
/// item, its place (`Place::NAMES` index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Grade(usize),
    Place(usize),
}

impl Param {
    /// Its name in the project.
    pub fn name(self) -> &'static str {
        match self {
            Param::Grade(k) => crate::grade::NAMES[k],
            Param::Place(k) => crate::overlay::Place::NAMES[k],
        }
    }

    /// As the panel calls it.
    pub fn title(self) -> &'static str {
        match self {
            Param::Grade(k) => crate::grade::EFFECTS.get(k).map_or("?", |e| e.label),
            Param::Place(k) => crate::overlay::Place::ABOUT[k].0,
        }
    }

    pub fn range(self) -> (f64, f64) {
        match self {
            Param::Grade(k) => crate::grade::RANGES[k],
            Param::Place(k) => crate::overlay::Place::RANGES[k],
        }
    }

    /// A value as the panel shows it.
    pub fn label(self, v: f64) -> String {
        match self {
            Param::Grade(k) => super::effects::label(k, v),
            Param::Place(3) => format!("{v:+.0}°"),
            Param::Place(_) => format!("{v:.2}"),
        }
    }
}

/// What is put on a cut or item for its whole length and set only in the
/// panel (a style, a LUT, a picture's kind): a row each under the Effects
/// row, a bar with its name, so what's applied shows on the timeline too
/// (the user, 2026-11-10). The rows' names, in their order.
pub const FIXED: [&str; 3] = ["Style", "LUT", "Kind"];

/// Cut `c`'s fixed effects: (row, what).
pub fn fixed_of_cut(c: &crate::project::Cut) -> Vec<(&'static str, String)> {
    let lut = c.lut.as_ref().map(|l| ("LUT", std::path::Path::new(l).file_stem().map_or_else(|| l.clone(), |s| s.to_string_lossy().into_owned())));
    c.style.as_ref().map(|s| ("Style", s.look.name().to_owned())).into_iter().chain(lut).collect()
}

/// Item `it`'s fixed effects: (row, what).
pub fn fixed_of_item(it: &crate::overlay::Item) -> Vec<(&'static str, String)> {
    let kind = it.kind.map(|k| ("Kind", k.name().to_owned()));
    it.style.as_ref().map(|s| ("Style", s.look.name().to_owned())).into_iter().chain(kind).collect()
}

/// The fixed rows `owners` need, in `FIXED`'s order.
pub fn fixed_rows<'a>(owners: impl Iterator<Item = &'a [(&'static str, String)]> + Clone) -> Vec<&'static str> {
    FIXED.into_iter().filter(|row| owners.clone().any(|f| f.iter().any(|(r, _)| r == row))).collect()
}

/// On `on`, value `param`; times are its clip (or own) time.
#[derive(Clone, Debug)]
pub enum AutoMsg {
    /// A clip drawn over `from`..`to`.
    Add { on: AutoOn, param: Param, from: f64, to: f64 },
    /// A point at `t` and `v`: the one at `from` moved there (a drag), or
    /// a new one.
    Point { on: AutoOn, param: Param, from: Option<f64>, t: f64, v: f64 },
    /// The clip pressed at `at` moved to start at `from` (a drag).
    Move { on: AutoOn, param: Param, at: f64, from: f64 },
    /// Its start or end to `t` (a drag let go: one step).
    Trim { on: AutoOn, param: Param, at: f64, start: bool, t: f64 },
    PointDelete { on: AutoOn, param: Param, t: f64 },
    /// The point at `t` eases on the next way: smooth, linear, hold.
    Ease { on: AutoOn, param: Param, t: f64 },
    Delete { on: AutoOn, param: Param, at: f64 },
}

impl App {
    /// The lanes of `of`: each value its clips move, or that the panel
    /// asked for (◆); an item's place first, then its effects.
    pub(super) fn lanes_of(&self, of: LaneOf) -> Vec<Param> {
        let clips: Vec<&crate::automation::AutoClip> = match of {
            LaneOf::Cuts => self.edit.cuts.iter().flat_map(|c| &c.automation).collect(),
            LaneOf::Track(track, l) => self.edit.lane(track, l).iter().flat_map(|l| &l.items).flat_map(|it| &it.automation).collect(),
        };
        let place = (0..4).map(Param::Place).filter(|_| of != LaneOf::Cuts);
        let all = place.chain((0..crate::grade::NAMES.len()).map(Param::Grade));
        all.filter(|p| clips.iter().any(|c| c.param == p.name()) || self.extra_lanes.contains(&(of, *p))).collect()
    }

    /// How many fixed rows the cuts' lanes have (`FIXED`).
    pub(super) fn fixed_cut_rows(&self) -> usize {
        let all: Vec<_> = self.edit.cuts.iter().map(fixed_of_cut).collect();
        fixed_rows(all.iter().map(|f| &f[..])).len()
    }

    /// `on` as an op names it, and its automation clips.
    fn auto_owner(&self, on: AutoOn) -> Option<(Graded, &[crate::automation::AutoClip])> {
        match on {
            AutoOn::Cut(i) => self.edit.cuts.get(i).map(|c| (Graded::Cut(c.id), &c.automation[..])),
            AutoOn::Item(id) => {
                Some((Graded::Item(id), &self.edit.item_by_id(id)?.automation[..]))
            }
        }
    }

    pub(super) fn on_auto(&mut self, message: AutoMsg) {
        let of = |on: AutoOn, param: Param| -> Option<(Graded, String)> { Some((self.auto_owner(on)?.0, param.name().to_owned())) };
        let op = match message {
            AutoMsg::Add { on, param, from, to } => {
                let Some((on, param)) = of(on, param) else { return };
                AutoOp::Add { on, param, from, to }
            }
            AutoMsg::Point { on, param, from, t, v } => {
                let Some((on, param)) = of(on, param) else { return };
                if from.is_some() {
                    self.edit.begin();
                }
                AutoOp::Point { on, param, t, v, from, tolerance: self.tolerance() }
            }
            AutoMsg::Move { on, param, at, from } => {
                let Some((on, param)) = of(on, param) else { return };
                self.edit.begin();
                AutoOp::Move { on, param, at, from }
            }
            AutoMsg::Trim { on, param, at, start, t } => {
                let Some((on, param)) = of(on, param) else { return };
                AutoOp::Trim { on, param, at, end: if start { End::In } else { End::Out }, t }
            }
            AutoMsg::PointDelete { on, param, t } => {
                let Some((on, param)) = of(on, param) else { return };
                AutoOp::PointDelete { on, param, t, tolerance: self.tolerance() }
            }
            AutoMsg::Ease { on, param, t } => {
                let tolerance = self.tolerance();
                let Some((graded, clips)) = self.auto_owner(on) else { return };
                let now = clips.iter().filter(|c| c.param == param.name() && c.covers(t)).flat_map(|c| &c.points).find(|q| (q.t - t).abs() <= tolerance).map(|q| q.ease);
                let ease = match now {
                    Some(Ease::Smooth) => Ease::Linear,
                    Some(Ease::Linear) => Ease::Hold,
                    _ => Ease::Smooth,
                };
                AutoOp::PointEase { on: graded, param: param.name().to_owned(), t, ease, tolerance }
            }
            AutoMsg::Delete { on, param, at } => {
                let Some((on, param)) = of(on, param) else { return };
                AutoOp::Delete { on, param, at }
            }
        };
        self.op(Op::Auto(op));
    }
}
