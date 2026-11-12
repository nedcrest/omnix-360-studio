//! Automation lanes (gap 39): under the cuts' Effects row, and under an
//! overlay or text lane's rows. One lane per value that something there
//! moves or the panel asked for (◆), each showing every owner's clips for
//! it — a block over its stretch, its curve, its points with their values
//! — the value's range the lane's height; under them a row for each thing
//! fixed on a whole owner and set in the panel (a style, a LUT, a
//! picture's kind), a named bar. One `Lanes` serves both canvases: its
//! owners are the cuts or the lane's items.

use super::*;
use crate::automation::AutoClip;
use super::super::automation_ui::{AutoMsg, AutoOn, Param};

/// A point's dot, half its side.
const DOT: f32 = 3.5;
/// A point's value tag's height.
const TAG_H: f32 = 12.0;

/// A run of automation lanes on a canvas.
pub(super) struct Lanes<'a> {
    pub t: &'a Timeline,
    pub params: Vec<Param>,
    /// The row they fold under («Effects», «Motion»): its name, its top
    /// and height (where its marks go while folded), and the first lane's
    /// top.
    pub name: &'static str,
    pub row_top: f32,
    pub row_h: f32,
    pub top: f32,
    pub folded: bool,
    pub owners: Vec<Owner<'a>>,
    /// The rows of what's fixed on the owners (`automation_ui::FIXED`),
    /// under the value lanes: a bar each, set in the panel only.
    pub fixed: Vec<&'static str>,
}

/// Whose clips a lane shows: where it starts on the edit, its own start
/// and length (clip or own time), its clips.
pub(super) struct Owner<'a> {
    pub on: AutoOn,
    pub from: f64,
    pub start: f64,
    pub len: f64,
    pub clips: &'a [AutoClip],
    /// What's fixed on it: (row, what), `automation_ui::fixed_of_cut`.
    pub fixed: Vec<(&'static str, String)>,
}

/// What a press on a lane meets (an owner by its place in `owners`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaneHit {
    Point { lane: usize, owner: usize, t: f64 },
    Value { lane: usize, owner: usize, t: f64, v: f64 },
    Edge { lane: usize, owner: usize, at: f64, start: bool },
    Clip { lane: usize, owner: usize, at: f64 },
    Empty { lane: usize, owner: usize },
}

impl LaneHit {
    pub(super) fn pointer(self) -> mouse::Interaction {
        use mouse::Interaction as I;
        match self {
            LaneHit::Empty { .. } => I::Crosshair,
            LaneHit::Point { .. } | LaneHit::Clip { .. } => I::Grab,
            LaneHit::Edge { .. } => I::ResizingHorizontally,
            LaneHit::Value { .. } => I::ResizingVertically,
        }
    }
}

/// A drag on a lane: a point (its time now), its value tag (from `y0`,
/// the value `from` then), a clip's edge (a time inside it, the edge's
/// time `to` now: trimmed once let go, its points hidden till then), a clip
/// pressed (moved once dragged a little, else a point added where it was
/// pressed), a clip drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaneDrag {
    Point { lane: usize, owner: usize, from: f64 },
    Value { lane: usize, owner: usize, t: f64, y0: f32, from: f64 },
    Edge { lane: usize, owner: usize, at: f64, start: bool, to: f64 },
    Body { lane: usize, owner: usize, at: f64, x0: f32, y0: f32, moved: bool },
    New { lane: usize, owner: usize, x0: f32, x1: f32 },
}

impl Lanes<'_> {
    /// Their height together: none while folded.
    pub(super) fn height(&self) -> f32 {
        if self.folded { 0.0 } else { self.rows() as f32 * AUTO }
    }

    /// Its lanes: the values', then the fixed rows.
    fn rows(&self) -> usize {
        self.params.len() + self.fixed.len()
    }

    /// Their row's name: «▾ Effects» open, «▸ Effects (2)» folded with two
    /// lanes under it.
    pub(super) fn tag(&self) -> String {
        match (self.folded, self.rows()) {
            (_, 0) => self.name.to_owned(),
            (true, n) => format!("▸ {} ({n})", self.name),
            (false, _) => format!("▾ {}", self.name),
        }
    }

    pub(super) fn lane(&self, i: usize) -> (f32, f32) {
        (self.top + i as f32 * AUTO, AUTO)
    }

    /// The height on lane `i` of value `v`, its range top to bottom.
    pub(super) fn y(&self, i: usize, v: f64) -> f32 {
        let (top, h) = self.lane(i);
        let (lo, hi) = self.params[i].range();
        top + h - 4.0 - ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 * (h - 8.0)
    }

    /// The value at height `y` on lane `i`.
    pub(super) fn value(&self, i: usize, y: f32) -> f64 {
        let (top, h) = self.lane(i);
        let (lo, hi) = self.params[i].range();
        lo + ((top + h - 4.0 - y) / (h - 8.0)).clamp(0.0, 1.0) as f64 * (hi - lo)
    }

    pub(super) fn x(&self, o: &Owner, t: f64, b: Rectangle) -> f32 {
        self.t.x(o.from + t - o.start, b)
    }

    /// `t` (own time) on `o`'s start or end when within `SNAP` pixels of
    /// it, as the timeline's edges snap (the user, 2026-11-10); free with
    /// snapping off or `free` (Shift).
    pub(super) fn snapped(&self, o: &Owner, t: f64, b: Rectangle, free: bool) -> f64 {
        if free || !self.t.snapping {
            return t;
        }
        let x = self.x(o, t, b);
        let near = |e: f64| (self.x(o, e, b) - x).abs();
        [o.start, o.start + o.len].into_iter().filter(|&e| near(e) < super::SNAP).min_by(|&a, &c| near(a).total_cmp(&near(c))).unwrap_or(t)
    }

    /// Owner `o`'s time at `x`.
    pub(super) fn own_time(&self, o: &Owner, x: f32, b: Rectangle) -> f64 {
        o.start + self.t.t(x, b) - o.from
    }

    /// `o`'s clips on lane `i`, with their edges in pixels cut to its stretch.
    fn clips<'b>(&'b self, i: usize, o: &'b Owner, b: Rectangle) -> impl Iterator<Item = (&'b AutoClip, f32, f32)> + 'b {
        let name = self.params[i].name();
        let (start, end) = (o.start, o.start + o.len);
        o.clips.iter().filter(move |a| a.param == name && a.to > start && a.from < end).map(move |a| (a, self.x(o, a.from.max(start), b), self.x(o, a.to.min(end), b)))
    }

    /// The open lane at height `y`.
    pub(super) fn lane_at(&self, y: f32) -> Option<usize> {
        if self.folded || y < self.top || y >= self.top + self.height() {
            return None;
        }
        Some(((y - self.top) / AUTO) as usize)
    }

    /// The point's value tag at (`x`, `y`) on lane `i`, kept in the lane.
    pub(super) fn value_tag(&self, i: usize, x: f32, y: f32, v: f64) -> Rectangle {
        let (top, h) = self.lane(i);
        let w = self.params[i].label(v).chars().count() as f32 * 5.6 + 6.0;
        Rectangle::new(Point::new(x + DOT + 2.0, (y - TAG_H / 2.0).clamp(top + 1.0, top + h - TAG_H - 1.0)), Size::new(w, TAG_H))
    }

    /// What a press at `p` meets: a value tag, a point, a clip's edge or
    /// body, or empty lane over an owner. Through a crossfade either cut's
    /// clips are found.
    pub(super) fn hit(&self, p: Point, b: Rectangle) -> Option<LaneHit> {
        // A fixed row is only shown: it's set in the panel.
        let lane = self.lane_at(p.y).filter(|&l| l < self.params.len())?;
        let reach = |o: &Owner| p.x >= self.t.x(o.from, b) - 4.0 && p.x <= self.t.x(o.from + o.len, b) + 4.0;
        for (owner, o) in self.owners.iter().enumerate().rev().filter(|(_, o)| reach(o)) {
            if let Some(hit) = self.clip_hit(lane, owner, o, p, b) {
                return Some(hit);
            }
        }
        let under = self.owners.iter().rposition(|o| p.x >= self.t.x(o.from, b) && p.x <= self.t.x(o.from + o.len, b))?;
        Some(LaneHit::Empty { lane, owner: under })
    }

    fn clip_hit(&self, lane: usize, owner: usize, o: &Owner, p: Point, b: Rectangle) -> Option<LaneHit> {
        for (clip, l, r) in self.clips(lane, o, b) {
            let shown = |q: &&crate::automation::AutoPoint| {
                let x = self.x(o, q.t, b);
                x >= l - 0.5 && x <= r + 0.5
            };
            if let Some(q) = clip.points.iter().filter(shown).find(|q| self.value_tag(lane, self.x(o, q.t, b), self.y(lane, q.v), q.v).contains(p)) {
                return Some(LaneHit::Value { lane, owner, t: q.t, v: q.v });
            }
            if let Some(q) = clip.points.iter().filter(shown).find(|q| (self.x(o, q.t, b) - p.x).abs() <= DOT + 2.0 && (self.y(lane, q.v) - p.y).abs() <= DOT + 2.0) {
                return Some(LaneHit::Point { lane, owner, t: q.t });
            }
            let at = (clip.from + clip.to) / 2.0;
            if (p.x - l).abs() <= 4.0 || (p.x - r).abs() <= 4.0 {
                return Some(LaneHit::Edge { lane, owner, at, start: (p.x - l).abs() <= (p.x - r).abs() });
            }
            if p.x > l && p.x < r {
                return Some(LaneHit::Clip { lane, owner, at: self.own_time(o, p.x, b) });
            }
        }
        None
    }

    /// A left press: the drag it starts, or (Ctrl on a point) the message.
    pub(super) fn press(&self, hit: LaneHit, p: Point, ctrl: bool) -> Result<LaneDrag, Message> {
        Ok(match hit {
            LaneHit::Point { lane, owner, t } if ctrl => return Err(Message::Auto(AutoMsg::PointDelete { on: self.owners[owner].on, param: self.params[lane], t })),
            LaneHit::Point { lane, owner, t } => LaneDrag::Point { lane, owner, from: t },
            LaneHit::Value { lane, owner, t, v } => LaneDrag::Value { lane, owner, t, y0: p.y, from: v },
            LaneHit::Edge { lane, owner, at, start } => {
                let clip = self.clip_at(lane, owner, at).ok_or(Message::ScrubDone)?;
                LaneDrag::Edge { lane, owner, at, start, to: if start { clip.from } else { clip.to } }
            }
            LaneHit::Clip { lane, owner, at } => LaneDrag::Body { lane, owner, at, x0: p.x, y0: p.y, moved: false },
            LaneHit::Empty { lane, owner } => LaneDrag::New { lane, owner, x0: p.x, x1: p.x },
        })
    }

    /// A right-click: a point's ease turned on, or a clip taken away.
    pub(super) fn right(&self, p: Point, b: Rectangle) -> Option<Message> {
        let of = |lane: usize, owner: usize| (self.owners[owner].on, self.params[lane]);
        match self.hit(p, b)? {
            LaneHit::Point { lane, owner, t } | LaneHit::Value { lane, owner, t, .. } => {
                let (on, param) = of(lane, owner);
                Some(Message::Auto(AutoMsg::Ease { on, param, t }))
            }
            LaneHit::Clip { lane, owner, at: t } | LaneHit::Edge { lane, owner, at: t, .. } => {
                let (on, param) = of(lane, owner);
                Some(Message::Auto(AutoMsg::Delete { on, param, at: t }))
            }
            LaneHit::Empty { .. } => None,
        }
    }

    /// The clip of lane `lane` on owner `owner` covering `at`.
    fn clip_at(&self, lane: usize, owner: usize, at: f64) -> Option<&AutoClip> {
        let name = self.params[lane].name();
        self.owners.get(owner)?.clips.iter().find(|a| a.param == name && a.covers(at))
    }

    /// A drag moved to `p`: the message it sends, its state kept on the
    /// clip as the clip moves. Shift held: a value tag's finer steps; Alt:
    /// free of snapping.
    pub(super) fn drag(&self, drag: &mut LaneDrag, p: Point, b: Rectangle, held: iced::keyboard::Modifiers) -> Option<Message> {
        let (fine, free) = (held.shift(), held.alt());
        match drag {
            LaneDrag::New { owner, x0, x1, .. } => {
                let o = &self.owners[*owner];
                let snap = |x: f32| self.x(o, self.snapped(o, self.own_time(o, x, b), b, free), b);
                (*x0, *x1) = (snap(*x0), snap(p.x));
                None
            }
            // Up is more: a 120th of the range a pixel in 200ths, Shift a
            // tenth of that in 1000ths, as a sound clip's dB tag drags.
            LaneDrag::Value { lane, owner, t, y0, from } => {
                let param = self.params[*lane];
                let (lo, hi) = param.range();
                let (per_px, step) = if fine { ((hi - lo) / 1200.0, (hi - lo) / 1000.0) } else { ((hi - lo) / 120.0, (hi - lo) / 200.0) };
                let v = (*from + ((*y0 - p.y) as f64 * per_px / step).round() * step).clamp(lo, hi);
                Some(Message::Auto(AutoMsg::Point { on: self.owners[*owner].on, param, from: Some(*t), t: *t, v }))
            }
            LaneDrag::Point { lane, owner, from } => {
                let clip = self.clip_at(*lane, *owner, *from)?;
                let o = &self.owners[*owner];
                let t = self.own_time(o, p.x, b).clamp(clip.from, clip.to);
                let message = AutoMsg::Point { on: o.on, param: self.params[*lane], from: Some(*from), t, v: self.value(*lane, p.y) };
                *from = t;
                Some(Message::Auto(message))
            }
            // Only drawn while dragged (the user, 2026-11-10): trimmed in one
            // step when let go, from the clip as it was.
            LaneDrag::Edge { lane, owner, at, start, to } => {
                let clip = self.clip_at(*lane, *owner, *at)?;
                let o = &self.owners[*owner];
                let shortest = crate::model::ops::auto::SHORTEST;
                let t = self.snapped(o, self.own_time(o, p.x, b), b, free);
                if let Some(t) = crate::automation::trim_to(*start, (clip.from, clip.to), (o.start, o.start + o.len), shortest, t) {
                    *to = t;
                }
                None
            }
            LaneDrag::Body { lane, owner, at, x0, moved, .. } => {
                if !*moved && (p.x - *x0).abs() <= 3.0 {
                    return None;
                }
                *moved = true;
                let clip = self.clip_at(*lane, *owner, *at)?;
                let o = &self.owners[*owner];
                let len = clip.to - clip.from;
                let from = clip.from + self.own_time(o, p.x, b) - self.own_time(o, *x0, b);
                // Its start onto an edge, else its end.
                let start = self.snapped(o, from, b, free);
                let from = if start != from { start } else { self.snapped(o, from + len, b, free) - len };
                let from = from.clamp(o.start, (o.start + o.len - len).max(o.start));
                let message = AutoMsg::Move { on: o.on, param: self.params[*lane], at: *at, from };
                *at += from - clip.from;
                // Only by what it moved: held on an edge, the pointer's
                // way past it adds up until it pulls free.
                *x0 += self.x(o, from, b) - self.x(o, clip.from, b);
                Some(Message::Auto(message))
            }
        }
    }

    /// A drag let go: a clip drawn, or a press on a clip that didn't move
    /// adding a point there; the rest close their undo step.
    pub(super) fn release(&self, drag: LaneDrag, b: Rectangle) -> Option<Message> {
        match drag {
            LaneDrag::New { lane, owner, x0, x1 } if (x1 - x0).abs() > 4.0 => {
                let o = &self.owners[owner];
                Some(Message::Auto(AutoMsg::Add { on: o.on, param: self.params[lane], from: self.own_time(o, x0, b), to: self.own_time(o, x1, b) }))
            }
            LaneDrag::New { .. } => None,
            // Let go where it was pressed: nothing trimmed (a click on the
            // edge of a clip hanging past its owner trimmed it, audit A1).
            LaneDrag::Edge { lane, owner, at, start, to } => {
                let clip = self.clip_at(lane, owner, at)?;
                let was = if start { clip.from } else { clip.to };
                ((to - was).abs() > 1e-9).then(|| Message::Auto(AutoMsg::Trim { on: self.owners[owner].on, param: self.params[lane], at, start, t: to }))
            }
            LaneDrag::Body { lane, owner, at, moved: false, y0, .. } => Some(Message::Auto(AutoMsg::Point { on: self.owners[owner].on, param: self.params[lane], from: None, t: at, v: self.value(lane, y0) })),
            _ => Some(Message::ScrubDone),
        }
    }

    /// The lanes: their ground, each owner's clips with their curves and
    /// points, and a rectangle being drawn; folded, marks in the Effects row.
    pub(super) fn draw(&self, frame: &mut Frame, theme: &Theme, b: Rectangle, drag: Option<&LaneDrag>) {
        if self.folded {
            self.draw_folded(frame, b);
            return;
        }
        let ground = theme.extended_palette().background.weak.color.scale_alpha(0.18);
        for i in 0..self.params.len() {
            let (top, h) = self.lane(i);
            frame.fill(&Path::rectangle(Point::new(0.0, top + 1.0), Size::new(b.width, h - 2.0)), ground);
            for (k, o) in self.owners.iter().enumerate() {
                for (clip, l, r) in self.clips(i, o, b) {
                    match drag {
                        Some(&LaneDrag::Edge { lane, owner, at, start, to }) if (lane, owner) == (i, k) && clip.covers(at) => {
                            let x = self.x(o, to, b);
                            self.clip_block(frame, i, if start { (x, r) } else { (l, x) });
                        }
                        _ => self.draw_clip(frame, i, o, clip, (l, r), b),
                    }
                }
            }
        }
        for (k, row) in self.fixed.iter().enumerate() {
            let i = self.params.len() + k;
            let (top, h) = self.lane(i);
            frame.fill(&Path::rectangle(Point::new(0.0, top + 1.0), Size::new(b.width, h - 2.0)), ground);
            for o in &self.owners {
                if let Some((_, what)) = o.fixed.iter().find(|(r, _)| r == row) {
                    self.fixed_bar(frame, i, (self.x(o, o.start, b), self.x(o, o.start + o.len, b)), what);
                }
            }
        }
        if let Some(&LaneDrag::New { lane, x0, x1, .. }) = drag {
            let (top, h) = self.lane(lane);
            frame.stroke(&Path::rectangle(Point::new(x0.min(x1), top + 2.0), Size::new((x1 - x0).abs(), h - 4.0)), Stroke::default().with_width(1.5).with_color(EFFECT_MARK));
        }
    }

    /// A fixed effect's bar on lane `i`, from `l` to `r`, named: no curve,
    /// no points, set in the panel.
    fn fixed_bar(&self, frame: &mut Frame, i: usize, (l, r): (f32, f32), what: &str) {
        let (top, h) = self.lane(i);
        let bar = Path::rectangle(Point::new(l, top + 4.0), Size::new((r - l).max(1.0), h - 8.0));
        frame.fill(&bar, EFFECT_MARK.scale_alpha(0.3));
        // Named where it has room.
        if r - l < 40.0 {
            return;
        }
        frame.fill_text(Text { content: what.to_owned(), position: Point::new(l + 6.0, top + h / 2.0), color: iced::Color::WHITE, size: 11.0.into(), align_y: Vertical::Center, ..Default::default() });
    }

    /// A clip's block on lane `i`, from `l` to `r`.
    fn clip_block(&self, frame: &mut Frame, i: usize, (l, r): (f32, f32)) {
        let (top, h) = self.lane(i);
        let block = Path::rectangle(Point::new(l, top + 2.0), Size::new((r - l).max(1.0), h - 4.0));
        frame.fill(&block, EFFECT_MARK.scale_alpha(0.18));
        frame.stroke(&block, Stroke::default().with_width(1.0).with_color(EFFECT_MARK.scale_alpha(0.7)));
    }

    fn draw_clip(&self, frame: &mut Frame, i: usize, o: &Owner, clip: &AutoClip, (l, r): (f32, f32), b: Rectangle) {
        self.clip_block(frame, i, (l, r));
        let curve = Path::new(|p| {
            let mut x = l;
            while x <= r {
                let y = self.y(i, clip.value_at(self.own_time(o, x, b)));
                if x == l { p.move_to(Point::new(x, y)) } else { p.line_to(Point::new(x, y)) }
                x += 3.0;
            }
        });
        frame.stroke(&curve, Stroke::default().with_width(1.5).with_color(EFFECT_MARK));
        for q in &clip.points {
            let x = self.x(o, q.t, b);
            if x >= l - 0.5 && x <= r + 0.5 {
                let y = self.y(i, q.v);
                frame.fill(&Path::rectangle(Point::new(x - DOT, y - DOT), Size::new(2.0 * DOT, 2.0 * DOT)), EFFECT_MARK);
                let tag = self.value_tag(i, x, y, q.v);
                frame.fill(&Path::rounded_rectangle(tag.position(), tag.size(), 2.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.65));
                frame.fill_text(Text { content: self.params[i].label(q.v), position: Point::new(tag.x + 3.0, tag.y + TAG_H / 2.0), color: iced::Color::WHITE, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
            }
        }
    }

    /// Folded, the Effects row marks where clips move a value and where
    /// their points are (the user, 2026-11-10: it was empty).
    fn draw_folded(&self, frame: &mut Frame, b: Rectangle) {
        let mid = self.row_top + self.row_h / 2.0;
        for o in &self.owners {
            if !o.fixed.is_empty() {
                frame.fill(&Path::rectangle(Point::new(self.x(o, o.start, b), mid - 1.0), Size::new((self.x(o, o.start + o.len, b) - self.x(o, o.start, b)).max(1.0), 2.0)), EFFECT_MARK.scale_alpha(0.3));
            }
            for i in 0..self.params.len() {
                for (clip, l, r) in self.clips(i, o, b) {
                    frame.fill(&Path::rectangle(Point::new(l, mid - 1.0), Size::new((r - l).max(1.0), 2.0)), EFFECT_MARK.scale_alpha(0.45));
                    for q in &clip.points {
                        let x = self.x(o, q.t, b);
                        if x >= l - 0.5 && x <= r + 0.5 {
                            frame.fill(&key_mark(x, mid, Ease::Smooth), EFFECT_MARK);
                        }
                    }
                }
            }
        }
    }

    /// The lanes' names in the header column.
    pub(super) fn draw_heads(&self, frame: &mut Frame, theme: &Theme) {
        if self.folded {
            return;
        }
        let ink = theme.extended_palette().background.base.text.scale_alpha(0.6);
        let names = self.params.iter().map(|p| p.title()).chain(self.fixed.iter().copied());
        for (i, name) in names.enumerate() {
            let (top, h) = self.lane(i);
            head_cell(frame, top, h, EFFECT_MARK);
            frame.fill_text(Text { content: name.to_owned(), position: Point::new(PICK + 14.0, top + h / 2.0), color: ink, size: 11.0.into(), align_y: Vertical::Center, ..Default::default() });
        }
    }

    /// Whether `p` is on the Effects row's name, folding the lanes (while
    /// there are any).
    pub(super) fn on_tag(&self, p: Point) -> bool {
        self.rows() > 0 && p.x < HEAD && fold_tag(&self.tag(), self.row_top + self.row_h / 2.0).contains(p)
    }
}

impl Timeline {
    /// The cuts' Effects row's lanes (false) or their Frame row's (true).
    pub(super) fn cut_group(&self, frame: bool) -> Lanes<'_> {
        if frame { self.frame_lanes() } else { self.cut_lanes() }
    }

    /// The cuts' Frame row (gap 107): a lane for each of the frame's
    /// values, all four once it's open, each showing the cuts' clips for it.
    pub(super) fn frame_lanes(&self) -> Lanes<'_> {
        // Ordinary cuts only: a 360 cut's frame is its view (the audit).
        let owners: Vec<Owner> = self.blocks.iter().enumerate().filter(|(_, c)| c.cut.flat).map(|(i, c)| Owner { on: AutoOn::Cut(i), from: c.from, start: c.cut.start, len: c.len(), clips: &c.cut.automation, fixed: Vec::new() }).collect();
        let params = (0..crate::frame::PARAMS.len()).map(Param::Frame).collect();
        Lanes { t: self, params, name: "Frame", row_top: self.rows.frame_top, row_h: EFFECTS, top: self.rows.frame_auto_top, folded: self.folds.frame, owners, fixed: Vec::new() }
    }

    /// The cuts' automation lanes, under their Effects row.
    pub(super) fn cut_lanes(&self) -> Lanes<'_> {
        let owners: Vec<Owner> = self.blocks.iter().enumerate().map(|(i, c)| Owner { on: AutoOn::Cut(i), from: c.from, start: c.cut.start, len: c.len(), clips: &c.cut.automation, fixed: super::super::automation_ui::fixed_of_cut(&c.cut) }).collect();
        let fixed = super::super::automation_ui::fixed_rows(owners.iter().map(|o| &o.fixed[..]));
        Lanes { t: self, params: self.auto_lanes.clone(), name: "Effects", row_top: self.rows.effects_top, row_h: EFFECTS, top: self.rows.auto_top, folded: self.folds.effects, owners, fixed }
    }
}
