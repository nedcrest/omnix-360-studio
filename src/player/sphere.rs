//! The view: the shader widget that draws the stitched (or flat) picture and
//! turns drags and the wheel into looking around (`Sphere`), and the box
//! drawn over it to pick a colour (`PickBox`).

use super::*;

/// The 360 view widget: draws the shader primitive and turns drags and the
/// scroll wheel into look and zoom messages.
#[derive(Clone)]
pub(super) struct Sphere {
    pub(super) rig: Arc<Rig>,
    pub(super) shown: Arc<Shown>,
    pub(super) camera: Camera,
    pub(super) rig_from_view: Vec<M3>,
    pub(super) fov_deg: f64,
    /// Drags draw a box to pick a colour instead of looking around.
    pub(super) picking: bool,
    /// The mouse turns, frames and zooms it: in Framing only; editing,
    /// the picture is left alone.
    pub(super) moves: bool,
    /// Through a crossfade, the next cut drawn over: its rig, frame,
    /// framing, readout views and how far in it is (0..1).
    pub(super) incoming: Option<gpu::Incoming>,
    /// Otherwise the cut's edge look now, and how much of it.
    pub(super) edge: Option<(crate::transition::Spec, f32)>,
    /// The cut's style at its clip time.
    pub(super) style: Option<(crate::transition::Spec, f32)>,
    pub(super) overlays: Vec<gpu::OverlayDraw>,
    /// The scopes, while they show (gap 69).
    pub(super) scope: Option<gpu::Scope>,
}

#[derive(Default)]
pub(super) struct Drag {
    pub(super) button: Option<mouse::Button>,
    pub(super) last: Option<Point>,
    /// The size last reported to the app.
    pub(super) size: Option<Size>,
    /// Where a colour-picking box started, in the view.
    pub(super) pick: Option<Point>,
    /// Where the press was, and whether it has moved from there: a press
    /// let go where it was is a click (it aims an ordinary cut's frame).
    pub(super) from: Option<Point>,
    pub(super) moved: bool,
}

impl Sphere {
    /// While picking a colour, a drag draws a box: the app hears of it as it
    /// grows and once it's drawn, in fractions of the view.
    pub(super) fn pick(&self, drag: &mut Drag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<shader::Action<Message>> {
        let at = || cursor.position().map(|p| Point::new((p.x - bounds.x).clamp(0.0, bounds.width), (p.y - bounds.y).clamp(0.0, bounds.height)));
        let area = |a: Point, b: Point| {
            Rectangle::new(Point::new(a.x.min(b.x) / bounds.width, a.y.min(b.y) / bounds.height), Size::new((a.x - b.x).abs() / bounds.width, (a.y - b.y).abs() / bounds.height))
        };
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor.position_over(bounds)?;
                drag.pick = at();
                Some(shader::Action::capture())
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (a, b) = (drag.pick?, at()?);
                Some(shader::Action::publish(Message::PickArea(area(a, b))).and_capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let (a, b) = (drag.pick.take()?, at()?);
                Some(shader::Action::publish(Message::AreaPicked(area(a, b), (bounds.width / bounds.height) as f64)).and_capture())
            }
            _ => None,
        }
    }

    /// Drags look around, the wheel zooms.
    pub(super) fn input(&self, drag: &mut Drag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<shader::Action<Message>> {
        if self.picking {
            return self.pick(drag, event, bounds, cursor);
        }
        if !self.moves {
            return None;
        }
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(b @ (mouse::Button::Left | mouse::Button::Right))) => {
                drag.last = Some(cursor.position_over(bounds)?);
                (drag.button, drag.from, drag.moved) = (Some(*b), drag.last, false);
                Some(shader::Action::capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(_)) if drag.button.is_some() => {
                let clicked = drag.button == Some(mouse::Button::Left) && !drag.moved;
                drag.button = None;
                // A click: where on the view, -1..1 each way (gap 107).
                let aim = drag.from.filter(|_| clicked).map(|p| Message::Aim([2.0 * ((p.x - bounds.x) / bounds.width) as f64 - 1.0, 2.0 * ((p.y - bounds.y) / bounds.height) as f64 - 1.0]));
                Some(shader::Action::publish(aim.unwrap_or(Message::LookDone)).and_capture())
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let (button, last) = (drag.button?, drag.last?);
                drag.last = Some(*position);
                drag.moved |= drag.from.is_some_and(|f| f.distance(*position) > 3.0);
                // Grab-style: the picture follows the pointer.
                let k = self.fov_deg.to_radians() / bounds.height as f64;
                let (dx, dy) = ((position.x - last.x) as f64, (position.y - last.y) as f64);
                let look = match button {
                    mouse::Button::Right => Message::Look { yaw: 0.0, pitch: 0.0, roll: dx * k },
                    _ => Message::Look { yaw: -dx * k, pitch: dy * k, roll: 0.0 },
                };
                Some(shader::Action::publish(look).and_capture())
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                cursor.position_over(bounds)?;
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y as f64,
                    mouse::ScrollDelta::Pixels { y, .. } => *y as f64 / 40.0,
                };
                Some(shader::Action::publish(Message::Zoom(-lines)).and_capture())
            }
            _ => None,
        }
    }
}

impl shader::Program<Message> for Sphere {
    type State = Drag;
    type Primitive = gpu::Primitive;

    fn update(&self, drag: &mut Drag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<shader::Action<Message>> {
        let action = self.input(drag, event, bounds, cursor);
        if action.is_none() && drag.size != Some(bounds.size()) {
            drag.size = Some(bounds.size());
            return Some(shader::Action::publish(Message::ViewSize(bounds.size())));
        }
        action
    }

    fn draw(&self, _: &Drag, _: mouse::Cursor, _: Rectangle) -> gpu::Primitive {
        gpu::Primitive {
            rig: self.rig.clone(),
            shown: self.shown.clone(),
            camera: self.camera,
            rig_from_view: self.rig_from_view.clone(),
            incoming: self.incoming.clone(),
            edge: self.edge,
            style: self.style,
            overlays: self.overlays.clone(),
            scope: self.scope.clone(),
        }
    }

    fn mouse_interaction(&self, drag: &Drag, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if self.picking && cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else if drag.button.is_some() {
            mouse::Interaction::Grabbing
        } else if self.moves && cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

/// The box being drawn to pick a colour, over the view.
pub(super) struct PickBox(pub(super) Option<Rectangle>);

impl canvas::Program<Message> for PickBox {
    type State = ();

    fn draw(&self, _: &(), renderer: &iced::Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<canvas::Geometry> {
        let Some(r) = &self.0 else { return vec![] };
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let rect = canvas::Path::rectangle(Point::new(r.x * bounds.width, r.y * bounds.height), Size::new(r.width * bounds.width, r.height * bounds.height));
        frame.fill(&rect, iced::Color { a: 0.15, ..iced::Color::WHITE });
        frame.stroke(&rect, canvas::Stroke::default().with_width(2.0).with_color(iced::Color::WHITE));
        vec![frame.into_geometry()]
    }
}
