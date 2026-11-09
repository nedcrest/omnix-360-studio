//! The automation lanes under the Effects row (gap 39): one per effect
//! that a cut moves or the user asked to (`Timeline::auto_lanes`, in
//! `grade::NAMES` order), each showing every cut's clips for it — a block
//! over its stretch, its curve and its points, the value's range the
//! lane's height (`grade::RANGES`).

use super::*;

/// A point's dot, half its side.
pub(super) const DOT: f32 = 3.5;
/// A point's value tag's height.
const TAG_H: f32 = 12.0;

impl Timeline {
    /// The Effects row's name: «▾ Effects» open, «▸ Effects (2)» folded
    /// with two lanes under it.
    pub(super) fn effects_tag(&self) -> String {
        match (self.folds.effects, self.auto_lanes.len()) {
            (_, 0) => "Effects".to_owned(),
            (true, n) => format!("▸ Effects ({n})"),
            (false, _) => "▾ Effects".to_owned(),
        }
    }

    /// Lane `i`'s top and height.
    pub(super) fn auto_lane(&self, i: usize) -> (f32, f32) {
        (self.rows.auto_top + i as f32 * AUTO, AUTO)
    }

    /// The height on lane `i` of value `v` of its effect, its range top
    /// to bottom.
    pub(super) fn auto_y(&self, i: usize, v: f64) -> f32 {
        let (top, h) = self.auto_lane(i);
        let (lo, hi) = crate::grade::RANGES[self.auto_lanes[i]];
        let k = ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
        top + h - 4.0 - k * (h - 8.0)
    }

    /// The value at height `y` on lane `i`.
    pub(super) fn auto_value(&self, i: usize, y: f32) -> f64 {
        let (top, h) = self.auto_lane(i);
        let (lo, hi) = crate::grade::RANGES[self.auto_lanes[i]];
        let k = ((top + h - 4.0 - y) / (h - 8.0)).clamp(0.0, 1.0) as f64;
        lo + k * (hi - lo)
    }

    /// Cut `c`'s clips on lane `i`: each with its left and right edge in
    /// pixels, cut to the cut's picture.
    pub(super) fn auto_clips<'a>(&'a self, i: usize, c: &'a Block, b: Rectangle) -> impl Iterator<Item = (usize, &'a crate::automation::AutoClip, f32, f32)> + 'a {
        let name = crate::grade::NAMES[self.auto_lanes[i]];
        let (start, end) = (c.cut.start, c.cut.start + c.len());
        c.cut.automation.iter().enumerate().filter(move |(_, a)| a.param == name && a.to > start && a.from < end).map(move |(k, a)| {
            let x = |t: f64| self.x(c.from + t - c.cut.start, b);
            (k, a, x(a.from.max(start)), x(a.to.min(end)))
        })
    }

    /// The lanes: their ground, then each cut's clips with their curves and
    /// points.
    pub(super) fn draw_auto(&self, frame: &mut Frame, theme: &Theme, b: Rectangle) {
        if self.folds.effects {
            self.draw_auto_folded(frame, b);
            return;
        }
        let ground = theme.extended_palette().background.weak.color.scale_alpha(0.18);
        for i in 0..self.auto_lanes.len() {
            let (top, h) = self.auto_lane(i);
            frame.fill(&Path::rectangle(Point::new(0.0, top + 1.0), Size::new(b.width, h - 2.0)), ground);
            for c in &self.blocks {
                for (_, clip, l, r) in self.auto_clips(i, c, b) {
                    let block = Path::rectangle(Point::new(l, top + 2.0), Size::new((r - l).max(1.0), h - 4.0));
                    frame.fill(&block, EFFECT_MARK.scale_alpha(0.18));
                    frame.stroke(&block, Stroke::default().with_width(1.0).with_color(EFFECT_MARK.scale_alpha(0.7)));
                    let curve = Path::new(|p| {
                        let mut x = l;
                        while x <= r {
                            let t = c.cut.start + (self.t(x, b) - c.from);
                            let y = self.auto_y(i, clip.value_at(t));
                            if x == l { p.move_to(Point::new(x, y)) } else { p.line_to(Point::new(x, y)) }
                            x += 3.0;
                        }
                    });
                    frame.stroke(&curve, Stroke::default().with_width(1.5).with_color(EFFECT_MARK));
                    for q in clip.points.iter().filter(|q| clip.covers(q.t)) {
                        let x = self.x(c.from + q.t - c.cut.start, b);
                        if x >= l - 0.5 && x <= r + 0.5 {
                            let y = self.auto_y(i, q.v);
                            frame.fill(&Path::rectangle(Point::new(x - DOT, y - DOT), Size::new(2.0 * DOT, 2.0 * DOT)), EFFECT_MARK);
                            self.draw_value_tag(frame, i, x, y, q.v);
                        }
                    }
                }
            }
        }
    }

    /// A point's value as the panel says it («+1.50 EV», «40 %»), what
    /// its tag shows.
    fn value_text(&self, i: usize, v: f64) -> String {
        super::super::effects::label(self.auto_lanes[i], v)
    }

    /// The tag beside a point at (`x`, `y`) on lane `i`, kept inside the
    /// lane: where it is, for drawing and for a press on it.
    pub(super) fn value_tag(&self, i: usize, x: f32, y: f32, v: f64) -> Rectangle {
        let (top, h) = self.auto_lane(i);
        let w = self.value_text(i, v).chars().count() as f32 * 5.6 + 6.0;
        let y = (y - TAG_H / 2.0).clamp(top + 1.0, top + h - TAG_H - 1.0);
        Rectangle::new(Point::new(x + DOT + 2.0, y), Size::new(w, TAG_H))
    }

    /// A point's value on a dark tag beside it (as a sound clip's dB).
    fn draw_value_tag(&self, frame: &mut Frame, i: usize, x: f32, y: f32, v: f64) {
        let r = self.value_tag(i, x, y, v);
        frame.fill(&Path::rounded_rectangle(r.position(), r.size(), 2.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.65));
        frame.fill_text(Text { content: self.value_text(i, v), position: Point::new(r.x + 3.0, r.y + TAG_H / 2.0), color: iced::Color::WHITE, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
    }

    /// Folded, the Effects row marks where clips move a value and where
    /// their points are (the user, 2026-11-10: it was empty).
    fn draw_auto_folded(&self, frame: &mut Frame, b: Rectangle) {
        let mid = self.rows.effects_top + EFFECTS / 2.0;
        for c in &self.blocks {
            for i in 0..self.auto_lanes.len() {
                for (_, clip, l, r) in self.auto_clips(i, c, b) {
                    frame.fill(&Path::rectangle(Point::new(l, mid - 1.0), Size::new((r - l).max(1.0), 2.0)), EFFECT_MARK.scale_alpha(0.45));
                    for q in clip.points.iter().filter(|q| clip.covers(q.t)) {
                        let x = self.x(c.from + q.t - c.cut.start, b);
                        if x >= l - 0.5 && x <= r + 0.5 {
                            frame.fill(&key_mark(x, mid, Ease::Smooth), EFFECT_MARK);
                        }
                    }
                }
            }
        }
    }

    /// The lanes' names in the header column.
    pub(super) fn draw_auto_heads(&self, frame: &mut Frame, theme: &Theme) {
        if self.folds.effects {
            return;
        }
        let ink = theme.extended_palette().background.base.text.scale_alpha(0.6);
        for (i, &p) in self.auto_lanes.iter().enumerate() {
            let (top, h) = self.auto_lane(i);
            head_cell(frame, top, h, EFFECT_MARK);
            let name = super::super::effects::PARAMS.get(p).map_or(crate::grade::NAMES[p], |(n, _)| n);
            frame.fill_text(Text { content: name.to_owned(), position: Point::new(PICK + 14.0, top + h / 2.0), color: ink, size: 11.0.into(), align_y: Vertical::Center, ..Default::default() });
        }
    }
}

impl Timeline {
    /// The open lane at height `y`.
    pub(super) fn auto_lane_at(&self, y: f32) -> Option<usize> {
        if self.folds.effects || y < self.rows.auto_top || y >= self.rows.auto_top + self.rows.auto {
            return None;
        }
        Some(((y - self.rows.auto_top) / AUTO) as usize).filter(|&i| i < self.auto_lanes.len())
    }

    /// Cut `c`'s clip time at `x`.
    pub(super) fn clip_time(&self, c: &Block, x: f32, b: Rectangle) -> f64 {
        c.cut.start + (self.t(x, b) - c.from)
    }

    /// What a press at `p` on lane `lane` meets: a point, a clip's edge, a
    /// clip, or empty lane over a cut.
    pub(super) fn auto_hit(&self, lane: usize, p: Point, b: Rectangle) -> Hit {
        let Some(under) = self.block_at(p.x, b) else { return Hit::Nothing };
        // Through a crossfade both cuts are under it: either one's clips.
        let reach = |c: &Block| p.x >= self.x(c.from, b) - 4.0 && p.x <= self.x(c.from + c.len(), b) + 4.0;
        for (cut, c) in self.blocks.iter().enumerate().rev().filter(|(_, c)| reach(c)) {
            if let Some(hit) = self.auto_clip_hit(lane, cut, c, p, b) {
                return hit;
            }
        }
        Hit::AutoEmpty { lane, cut: under }
    }

    /// A point, an edge or the body of one of cut `c`'s clips on lane `lane`.
    fn auto_clip_hit(&self, lane: usize, cut: usize, c: &Block, p: Point, b: Rectangle) -> Option<Hit> {
        for (_, clip, l, r) in self.auto_clips(lane, c, b) {
            let on_tag = clip.points.iter().find(|q| {
                let x = self.x(c.from + q.t - c.cut.start, b);
                x >= l - 0.5 && x <= r + 0.5 && self.value_tag(lane, x, self.auto_y(lane, q.v), q.v).contains(p)
            });
            if let Some(q) = on_tag {
                return Some(Hit::AutoValue { lane, cut, t: q.t, v: q.v });
            }
            let near = clip.points.iter().find(|q| (self.x(c.from + q.t - c.cut.start, b) - p.x).abs() <= DOT + 2.0 && (self.auto_y(lane, q.v) - p.y).abs() <= DOT + 2.0);
            if let Some(q) = near {
                return Some(Hit::AutoPoint { lane, cut, t: q.t });
            }
            let at = (clip.from + clip.to) / 2.0;
            if (p.x - l).abs() <= 4.0 || (p.x - r).abs() <= 4.0 {
                return Some(Hit::AutoEdge { lane, cut, at, start: (p.x - l).abs() <= (p.x - r).abs() });
            }
            if p.x > l && p.x < r {
                return Some(Hit::AutoClip { lane, cut, at: self.clip_time(c, p.x, b) });
            }
        }
        None
    }

    /// A right-click on lane `lane`: a point's ease turned on, or a clip
    /// taken away.
    pub(super) fn auto_right(&self, lane: usize, p: Point, b: Rectangle) -> Option<Message> {
        use super::super::automation_ui::AutoMsg;
        let param = self.auto_lanes[lane];
        match self.auto_hit(lane, p, b) {
            Hit::AutoPoint { cut, t, .. } | Hit::AutoValue { cut, t, .. } => Some(Message::Auto(AutoMsg::Ease { cut, param, t })),
            Hit::AutoClip { cut, at, .. } | Hit::AutoEdge { cut, at, .. } => Some(Message::Auto(AutoMsg::Delete { cut, param, at })),
            _ => None,
        }
    }

    /// The clip of lane `lane`'s effect on cut `cut` covering clip time `at`.
    pub(super) fn auto_clip(&self, lane: usize, cut: usize, at: f64) -> Option<&crate::automation::AutoClip> {
        let name = crate::grade::NAMES[self.auto_lanes[lane]];
        self.blocks.get(cut)?.cut.automation.iter().find(|a| a.param == name && a.covers(at))
    }

    /// A lane drag moved to `p`: the message it sends, its state kept on
    /// the clip as the clip moves.
    pub(super) fn auto_drag(&self, drag: &mut Drag, p: Point, b: Rectangle, shift: bool) -> Option<Message> {
        use super::super::automation_ui::AutoMsg;
        match drag {
            // Up is more: a 120th of the range a pixel in 200ths, Shift a
            // tenth of that in 1000ths, as a sound clip's dB tag drags.
            Drag::AutoValue { lane, cut, t, y0, from } => {
                let (lo, hi) = crate::grade::RANGES[self.auto_lanes[*lane]];
                let (per_px, step) = if shift { ((hi - lo) / 1200.0, (hi - lo) / 1000.0) } else { ((hi - lo) / 120.0, (hi - lo) / 200.0) };
                let v = (*from + ((*y0 - p.y) as f64 * per_px / step).round() * step).clamp(lo, hi);
                Some(Message::Auto(AutoMsg::Point { cut: *cut, param: self.auto_lanes[*lane], from: Some(*t), t: *t, v }))
            }
            Drag::AutoPoint { lane, cut, from } => {
                let (lane, cut) = (*lane, *cut);
                let clip = self.auto_clip(lane, cut, *from)?;
                let t = self.clip_time(&self.blocks[cut], p.x, b).clamp(clip.from, clip.to);
                let message = AutoMsg::Point { cut, param: self.auto_lanes[lane], from: Some(*from), t, v: self.auto_value(lane, p.y) };
                *from = t;
                Some(Message::Auto(message))
            }
            Drag::AutoEdge { lane, cut, at, start } => {
                let (lane, cut, start) = (*lane, *cut, *start);
                let clip = self.auto_clip(lane, cut, *at)?;
                let shortest = crate::model::ops::auto::SHORTEST;
                let c = &self.blocks[cut];
                let t = self.clip_time(c, p.x, b);
                let (from, to) = if start { (t.clamp(c.cut.start, clip.to - shortest), clip.to) } else { (clip.from, t.clamp(clip.from + shortest, c.cut.start + c.len())) };
                let message = AutoMsg::Trim { cut, param: self.auto_lanes[lane], at: *at, start, t: if start { from } else { to } };
                *at = (from + to) / 2.0;
                Some(Message::Auto(message))
            }
            Drag::AutoBody { lane, cut, at, x0, moved, .. } => {
                let (lane, cut) = (*lane, *cut);
                if !*moved && (p.x - *x0).abs() <= 3.0 {
                    return None;
                }
                *moved = true;
                let clip = self.auto_clip(lane, cut, *at)?;
                let c = &self.blocks[cut];
                let len = clip.to - clip.from;
                let from = (clip.from + self.clip_time(c, p.x, b) - self.clip_time(c, *x0, b)).clamp(c.cut.start, (c.cut.start + c.len() - len).max(c.cut.start));
                let message = AutoMsg::Move { cut, param: self.auto_lanes[lane], at: *at, from };
                *at += from - clip.from;
                *x0 = p.x;
                Some(Message::Auto(message))
            }
            _ => None,
        }
    }

    /// A lane drag let go: a clip drawn, or a press on a clip that didn't
    /// move adding a point there; the rest close their undo step.
    pub(super) fn auto_release(&self, drag: Drag, b: Rectangle) -> Option<Message> {
        use super::super::automation_ui::AutoMsg;
        match drag {
            Drag::AutoNew { lane, cut, x0, x1 } if (x1 - x0).abs() > 4.0 => {
                let c = &self.blocks[cut];
                Some(Message::Auto(AutoMsg::Add { cut, param: self.auto_lanes[lane], from: self.clip_time(c, x0, b), to: self.clip_time(c, x1, b) }))
            }
            Drag::AutoBody { lane, cut, at, moved: false, y0, .. } => Some(Message::Auto(AutoMsg::Point { cut, param: self.auto_lanes[lane], from: None, t: at, v: self.auto_value(lane, y0) })),
            Drag::AutoPoint { .. } | Drag::AutoEdge { .. } | Drag::AutoBody { .. } | Drag::AutoValue { .. } => Some(Message::ScrubDone),
            _ => None,
        }
    }

    /// A rectangle being drawn on a lane.
    pub(super) fn draw_auto_new(&self, frame: &mut Frame, drag: &Drag) {
        if let Drag::AutoNew { lane, x0, x1, .. } = *drag {
            let (top, h) = self.auto_lane(lane);
            frame.stroke(&Path::rectangle(Point::new(x0.min(x1), top + 2.0), Size::new((x1 - x0).abs(), h - 4.0)), Stroke::default().with_width(1.5).with_color(EFFECT_MARK));
        }
    }
}
