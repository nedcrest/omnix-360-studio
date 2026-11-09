//! The automation lanes' messages (gap 39): what a press or a drag on a
//! lane under the Effects row does, made into `Op::Auto` on the cut it's
//! over. Drags are one undo step each, closed on release (`ScrubDone`).

use super::App;
use crate::model::ops::keys::Graded;
use crate::model::ops::{End, Op, auto::AutoOp};
use crate::view::Ease;

/// On cut `cut` (its index), effect `param` (`grade::NAMES` index); times
/// are the cut's clip time.
#[derive(Clone, Debug)]
pub enum AutoMsg {
    /// A clip drawn over `from`..`to`.
    Add { cut: usize, param: usize, from: f64, to: f64 },
    /// A point at `t` and `v`: the one at `from` moved there (a drag), or
    /// a new one.
    Point { cut: usize, param: usize, from: Option<f64>, t: f64, v: f64 },
    /// The clip pressed at `at` moved to start at `from` (a drag).
    Move { cut: usize, param: usize, at: f64, from: f64 },
    /// Its start or end to `t` (a drag).
    Trim { cut: usize, param: usize, at: f64, start: bool, t: f64 },
    PointDelete { cut: usize, param: usize, t: f64 },
    /// The point at `t` eases on the next way: smooth, linear, hold.
    Ease { cut: usize, param: usize, t: f64 },
    Delete { cut: usize, param: usize, at: f64 },
}

impl App {
    pub(super) fn on_auto(&mut self, message: AutoMsg) {
        let of = |cut: usize, param: usize| -> Option<(Graded, String)> { Some((Graded::Cut(self.edit.cuts.get(cut)?.id), crate::grade::NAMES.get(param)?.to_string())) };
        let op = match message {
            AutoMsg::Add { cut, param, from, to } => {
                let Some((on, param)) = of(cut, param) else { return };
                AutoOp::Add { on, param, from, to }
            }
            AutoMsg::Point { cut, param, from, t, v } => {
                let Some((on, param)) = of(cut, param) else { return };
                if from.is_some() {
                    self.edit.begin();
                }
                AutoOp::Point { on, param, t, v, from, tolerance: self.tolerance() }
            }
            AutoMsg::Move { cut, param, at, from } => {
                let Some((on, param)) = of(cut, param) else { return };
                self.edit.begin();
                AutoOp::Move { on, param, at, from }
            }
            AutoMsg::Trim { cut, param, at, start, t } => {
                let Some((on, param)) = of(cut, param) else { return };
                self.edit.begin();
                AutoOp::Trim { on, param, at, end: if start { End::In } else { End::Out }, t }
            }
            AutoMsg::PointDelete { cut, param, t } => {
                let Some((on, param)) = of(cut, param) else { return };
                AutoOp::PointDelete { on, param, t, tolerance: self.tolerance() }
            }
            AutoMsg::Ease { cut, param, t } => {
                let Some((on, name)) = of(cut, param) else { return };
                let tolerance = self.tolerance();
                let now = self.edit.cuts[cut].automation.iter().filter(|c| c.param == name && c.covers(t)).flat_map(|c| &c.points).find(|q| (q.t - t).abs() <= tolerance).map(|q| q.ease);
                let ease = match now {
                    Some(Ease::Smooth) => Ease::Linear,
                    Some(Ease::Linear) => Ease::Hold,
                    _ => Ease::Smooth,
                };
                AutoOp::PointEase { on, param: name, t, ease, tolerance }
            }
            AutoMsg::Delete { cut, param, at } => {
                let Some((on, param)) = of(cut, param) else { return };
                AutoOp::Delete { on, param, at }
            }
        };
        self.op(Op::Auto(op));
    }
}
