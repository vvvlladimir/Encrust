use super::{BREAK_W, CAPTION, FIGURE_H, INSET, LANE, LANE_STEP, MARGIN, WALL_W};
use egui::{Pos2, Rect, Vec2, pos2};

/// A side view at one scale: millimetres along the support from the model's face (x) and from
/// its axis (y), to points. A cut draws `BREAK_W` wide in place of a stretch.
pub(super) struct View {
    pub(super) frame: Rect,
    pub(super) origin: Pos2,
    pub(super) scale: f32,
    pub(super) cuts: Vec<(f32, f32)>,
    pub(super) standing: bool,
}

impl View {
    /// The scale fitting `along` lengthwise less its cuts, and `half_mm` either side of the axis;
    /// where height limits it, the first cut gives back the width's spare.
    pub(super) fn fit(frame: Rect, along: (f32, f32), half_mm: f32, cuts: Vec<(f32, f32)>) -> Self {
        let room_h = frame.height() - 2.0 * (INSET + MARGIN) - CAPTION;
        let axis = frame.center().y + CAPTION / 2.0;
        Self::scaled(frame, along, 2.0 * half_mm, room_h, axis, cuts)
    }

    /// The support standing on the plate, which runs along the frame's foot: `along` fits
    /// the height, with its end on the plate, and `across` the width, centred.
    pub(super) fn standing(frame: Rect, along: (f32, f32), across: (f32, f32)) -> Self {
        let plate = frame.bottom() - INSET - WALL_W;
        let room_h = plate - (frame.top() + INSET + CAPTION + FIGURE_H);
        let room_w = frame.width() - 2.0 * (INSET + MARGIN + FIGURE_H);
        let scale =
            (room_h / (along.1 - along.0).max(1e-3)).min(room_w / (across.1 - across.0).max(1e-3));
        let centre = frame.center().x - (across.0 + across.1) / 2.0 * scale;
        Self {
            frame,
            origin: pos2(centre, plate - along.1 * scale),
            scale,
            cuts: Vec::new(),
            standing: true,
        }
    }

    pub(super) fn scaled(
        frame: Rect,
        along: (f32, f32),
        across_mm: f32,
        room_h: f32,
        axis: f32,
        mut cuts: Vec<(f32, f32)>,
    ) -> Self {
        let cut_mm: f32 = cuts.iter().map(|(from, to)| to - from).sum();
        let room_w = frame.width() - 2.0 * INSET - BREAK_W * cuts.len() as f32;
        let length = (along.1 - along.0 - cut_mm).max(1e-3);
        let scale = (room_w / length).min(room_h / across_mm.max(1e-3));
        if let Some(first) = cuts.first_mut() {
            first.0 = (first.0 + (room_w / scale - length).max(0.0)).min(first.1);
        }
        cuts.retain(|(from, to)| to - from > 1e-4);
        Self {
            frame,
            origin: pos2(frame.left() + INSET - along.0 * scale, axis),
            scale,
            cuts,
            standing: false,
        }
    }

    /// The same view moved along so that `from_mm` to `to_mm`, and a wall's width after it,
    /// stand in the middle of the frame.
    pub(super) fn centred(mut self, from_mm: f32, to_mm: f32) -> Self {
        let width = self.x(to_mm) + WALL_W - self.x(from_mm);
        let spare = (self.frame.width() - 2.0 * INSET - width).max(0.0);
        self.origin.x += self.frame.left() + INSET + spare / 2.0 - self.x(from_mm);
        self
    }

    /// The same view moved along so that `x_mm` stands `gap` points in from the right.
    pub(super) fn anchored_right(mut self, x_mm: f32, gap: f32) -> Self {
        self.origin.x += self.frame.right() - INSET - gap - self.x(x_mm);
        self
    }

    /// Points along the support from its origin to `x_mm`, less what the cuts take out.
    pub(super) fn along(&self, x_mm: f32) -> f32 {
        let mut pt = x_mm * self.scale;
        for &(from, to) in &self.cuts {
            let inside = (x_mm.min(to) - from).max(0.0);
            pt -= inside * self.scale - BREAK_W * inside / (to - from);
        }
        pt
    }

    pub(super) fn x(&self, x_mm: f32) -> f32 {
        self.origin.x + self.along(x_mm)
    }

    /// Which way on screen the support runs, and which way across it.
    pub(super) fn along_dir(&self) -> Vec2 {
        if self.standing { Vec2::Y } else { Vec2::X }
    }

    pub(super) fn across_dir(&self) -> Vec2 {
        if self.standing { Vec2::X } else { Vec2::Y }
    }

    /// The scale a lying view of `along_mm` lengthwise and `across_mm` across would get
    /// in `frame`, and the same standing.
    pub(super) fn scales(frame: Rect, along_mm: f32, across_mm: f32) -> (f32, f32) {
        let lying = Self::fit(frame, (0.0, along_mm), across_mm / 2.0, Vec::new()).scale;
        let standing = Self::standing(frame, (0.0, along_mm), (0.0, across_mm)).scale;
        (lying, standing)
    }

    pub(super) fn y(&self, y_mm: f32) -> f32 {
        self.origin.y + y_mm * self.scale
    }

    pub(super) fn at(&self, x_mm: f32, y_mm: f32) -> Pos2 {
        if self.standing {
            pos2(
                self.origin.x + y_mm * self.scale,
                self.origin.y + self.along(x_mm),
            )
        } else {
            pos2(self.x(x_mm), self.y(y_mm))
        }
    }

    pub(super) fn len(&self, mm: f32) -> f32 {
        mm * self.scale
    }

    /// The `row`th row of figures above the axis, clear of `reach` points either side of
    /// it; a negative row is below.
    pub(super) fn lane(&self, reach: f32, row: i32) -> f32 {
        let off = reach + LANE + LANE_STEP * (row.abs() - 1) as f32;
        if row > 0 {
            self.origin.y - off
        } else {
            self.origin.y + off
        }
    }
}
