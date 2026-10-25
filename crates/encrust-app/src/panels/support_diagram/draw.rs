use super::column::{Station, half_at, station};
use super::view::View;
use super::{ARROW, CAPTION, FITS, HATCH_GAP, HEIGHT, INSET, LINE_W, Part, WALL_W, free_spot};
use crate::i18n::tr;
use crate::ui::theme;
use egui::{Align2, Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

/// How one outline is drawn: the accent over a wash when it is the part picked, a plain
/// line otherwise.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pen {
    Picked,
    Plain,
}

impl Pen {
    pub(super) fn of(part: Part, picked: Part) -> Self {
        if part == picked {
            Self::Picked
        } else {
            Self::Plain
        }
    }

    pub(super) fn stroke(self) -> Stroke {
        let colors = theme::colors();
        let colour = match self {
            Self::Picked => colors.accent,
            Self::Plain => colors.text_low,
        };
        Stroke::new(LINE_W, colour)
    }

    /// The wash as it lands on the drawing's ground, so pieces that overlap read as one.
    pub(super) fn fill(self) -> Color32 {
        let colors = theme::colors();
        match self {
            Self::Picked => colors.sunken.blend(colors.accent_wash),
            Self::Plain => Color32::TRANSPARENT,
        }
    }

    /// A convex outline, washed and stroked, or only dashed when it is switched off.
    pub(super) fn outline(self, points: Vec<Pos2>, off: bool) -> Vec<Shape> {
        if off {
            let mut closed = points.clone();
            closed.extend(points.first().copied());
            return Shape::dashed_line(&closed, self.stroke(), 4.0, 3.0);
        }
        vec![Shape::convex_polygon(points, self.fill(), self.stroke())]
    }
}

/// The body swept about an axis `offset_mm` off the view's, each stretch in its part's
/// pen, with `picked` in the accent.
pub(super) fn revolved(
    painter: &Painter,
    view: &View,
    stations: &[Station],
    offset_mm: f32,
    picked: Part,
) {
    let pen = |part: Part| Pen::of(part, picked);
    draw_revolved(painter, view, stations, offset_mm, &pen, false);
}

/// `stations` as bands washed one by one, their edges stroked, and a line across wherever
/// one part gives way to another and at both ends; only the edges, dashed, when `off`.
pub(super) fn draw_revolved(
    painter: &Painter,
    view: &View,
    stations: &[Station],
    offset_mm: f32,
    pen: &dyn Fn(Part) -> Pen,
    off: bool,
) {
    let stations = split_at_cuts(stations, &view.cuts);
    let point = |x: f32, half: f32| view.at(x, offset_mm + half);
    for pair in stations.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let pen = pen(a.part);
        let upper = [point(a.x, -a.half), point(b.x, -b.half)];
        let lower = [point(a.x, a.half), point(b.x, b.half)];
        if off {
            painter.extend(Shape::dashed_line(&upper, pen.stroke(), 4.0, 3.0));
            painter.extend(Shape::dashed_line(&lower, pen.stroke(), 4.0, 3.0));
            continue;
        }
        let band = vec![upper[0], upper[1], lower[1], lower[0]];
        painter.add(Shape::convex_polygon(band, pen.fill(), Stroke::NONE));
        painter.line_segment(upper, pen.stroke());
        painter.line_segment(lower, pen.stroke());
    }
    for (index, here) in stations.iter().enumerate() {
        let before = index.checked_sub(1).map(|index| stations[index]);
        let turns = before.is_some_and(|before| before.part != here.part);
        if index == 0 || index + 1 == stations.len() || turns {
            let part = before.map_or(here.part, |before| before.part);
            let ends = [point(here.x, -here.half), point(here.x, here.half)];
            painter.line_segment(ends, pen(part).stroke());
        }
    }
}

/// `stations` with one added wherever a cut begins or ends inside them, so a band never
/// spans a cut.
pub(super) fn split_at_cuts(stations: &[Station], cuts: &[(f32, f32)]) -> Vec<Station> {
    let mut split = Vec::with_capacity(stations.len() + 2 * cuts.len());
    for pair in stations.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        split.push(a);
        let mut inside: Vec<f32> = cuts
            .iter()
            .flat_map(|&(from, to)| [from, to])
            .filter(|&x| x > a.x && x < b.x)
            .collect();
        inside.sort_by(f32::total_cmp);
        for x in inside {
            split.push(station(x, half_at(&[a, b], x), a.part));
        }
    }
    split.extend(stations.last().copied());
    split
}

/// The marks of every cut across the body: a gap of the ground with a zigzag either side.
pub(super) fn breaks(painter: &Painter, view: &View, stations: &[Station]) {
    let colors = theme::colors();
    for &(from, to) in &view.cuts {
        let half = view.len(half_at(stations, from).max(half_at(stations, to))) + 4.0;
        let (left, right) = (view.x(from) + 2.0, view.x(to) - 2.0);
        let y = view.origin.y;
        let gap = Rect::from_x_y_ranges(left..=right, y - half..=y + half);
        painter.rect_filled(gap, 0.0, colors.sunken);
        for x in [left, right] {
            let zigzag: Vec<Pos2> = (0..=6)
                .map(|step| {
                    let along = -half + 2.0 * half * step as f32 / 6.0;
                    let wobble = if step % 2 == 0 { -2.0 } else { 2.0 };
                    pos2(x + wobble, y + along)
                })
                .collect();
            painter.add(Shape::line(zigzag, Stroke::new(1.0, colors.text_low)));
        }
    }
}

/// Takes the drawing's room out of the panel and frames it.
pub(super) fn canvas(ui: &mut Ui) -> Rect {
    let (frame, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    let colors = theme::colors();
    ui.painter().rect(
        frame,
        theme::R_SURFACE,
        colors.sunken,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Inside,
    );
    frame
}

/// The model's face where the support starts, hatched out to the frame's left.
pub(super) fn model_wall(painter: &Painter, view: &View) {
    let face = view.x(0.0);
    let left = view.frame.left() + INSET;
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(left.min(face - WALL_W)..=face, top..=bottom),
    );
    let at = pos2(left, view.frame.top() + INSET);
    caption(painter, at, Align2::LEFT_TOP, &tr!("diagram-model"));
}

/// A face of the model at `x_mm` hatched out to the frame's right, with its caption.
pub(super) fn far_wall(painter: &Painter, view: &View, x_mm: f32) {
    let face = view.x(x_mm);
    let right = view.frame.right() - INSET;
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(face..=right.max(face + WALL_W), top..=bottom),
    );
    let at = pos2(right, view.frame.top() + INSET);
    caption(painter, at, Align2::RIGHT_TOP, &tr!("diagram-model"));
}

/// The plate at `x_mm`, a wall's width thick, with its caption over it.
pub(super) fn plate_wall(painter: &Painter, view: &View, x_mm: f32) {
    let face = view.x(x_mm);
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(face..=face + WALL_W, top..=bottom),
    );
    let at = pos2(face + WALL_W, view.frame.top() + INSET);
    caption(painter, at, Align2::RIGHT_TOP, &tr!("diagram-plate"));
}

/// The plate under a standing support, hatched down to the frame's foot.
pub(super) fn plate_floor(painter: &Painter, view: &View, x_mm: f32) {
    let top = view.at(x_mm, 0.0).y;
    let (left, right) = (view.frame.left() + INSET, view.frame.right() - INSET);
    let bottom = (view.frame.bottom() - INSET).max(top + WALL_W);
    hatch(painter, Rect::from_x_y_ranges(left..=right, top..=bottom));
    // Written inside the floor, on a patch, where nothing above it can reach.
    let at = pos2(right - 4.0, (top + bottom) / 2.0);
    let (font, colour) = (theme::figures(10.0), theme::colors().text_low);
    let galley = painter.layout_no_wrap(tr!("diagram-plate"), font, colour);
    let rect = Align2::RIGHT_CENTER.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand2(vec2(3.0, 0.0)), 2.0, theme::colors().sunken);
    painter.galley(rect.min, galley, colour);
}

/// The span the walls stand over, top to bottom, leaving the captions their row.
pub(super) fn wall_span(frame: Rect) -> (f32, f32) {
    (frame.top() + INSET + CAPTION, frame.bottom() - INSET)
}

pub(super) fn caption(painter: &Painter, at: Pos2, align: Align2, label: &str) {
    let (font, colour) = (theme::figures(10.0), theme::colors().text_low);
    painter.text(at, align, label, font, colour);
}

/// A wall the drawing stands against, as a draughtsman hatches a section.
pub(super) fn hatch(painter: &Painter, rect: Rect) {
    let colors = theme::colors();
    let clipped = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let stroke = Stroke::new(1.0, colors.line);
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        let from = pos2(x, rect.bottom());
        clipped.line_segment([from, from + vec2(rect.height(), -rect.height())], stroke);
        x += HATCH_GAP;
    }
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(LINE_W, colors.text_low),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn accent_line() -> Stroke {
    Stroke::new(1.0, theme::colors().accent)
}

/// A measured length between `from` and `to`, arrowed at both ends, its figure over its
/// middle when it has one.
pub(super) fn dimension(painter: &Painter, from: Pos2, to: Pos2, label: &str) {
    let length = (to - from).length();
    if length < ARROW * 3.0 {
        // Too short for its own heads, so it is pointed at from outside.
        let out = (to - from) / length.max(f32::EPSILON) * ARROW * 2.5;
        pointed(painter, from - out, from);
        pointed(painter, to + out, to);
    } else {
        painter.line_segment([from, to], accent_line());
        arrowhead(painter, from, to);
        arrowhead(painter, to, from);
    }
    if label.is_empty() {
        return;
    }
    let middle = from.lerp(to, 0.5);
    let level = (to - from).x.abs() >= (to - from).y.abs();
    // A figure that would cover most of its line stands over it instead.
    let wide = label_width(painter, label) > length * 0.6;
    match (level, wide) {
        (true, false) => text(painter, middle, Align2::CENTER_CENTER, label),
        (true, true) => text(
            painter,
            middle - vec2(0.0, 2.0),
            Align2::CENTER_BOTTOM,
            label,
        ),
        (false, _) => text(
            painter,
            middle + vec2(-4.0, 0.0),
            Align2::RIGHT_CENTER,
            label,
        ),
    }
}

pub(super) fn label_width(painter: &Painter, label: &str) -> f32 {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    galley.size().x
}

/// An arrow from `from` to `to` with its head at `to`.
pub(super) fn pointed(painter: &Painter, from: Pos2, to: Pos2) {
    painter.line_segment([from, to], accent_line());
    arrowhead(painter, to, from);
}

/// A diameter: the arrow across `half` points either side of `centre`. Its figure sits beside
/// it, aligned by `inside`, when it fits; else at `outside` with a leader.
pub(super) fn across(
    painter: &Painter,
    centre: Pos2,
    half: f32,
    label: &str,
    inside: Option<Align2>,
    outside: (Pos2, Align2),
) {
    let (from, to) = (centre - vec2(0.0, half), centre + vec2(0.0, half));
    dimension(painter, from, to, "");
    if let Some(align) = inside.filter(|_| half * 2.0 >= FITS) {
        let side = if align == Align2::RIGHT_CENTER {
            -4.0
        } else {
            4.0
        };
        text(painter, centre + vec2(side, 0.0), align, label);
        return;
    }
    let (at, align) = outside;
    let end = if at.y < centre.y { from } else { to };
    let leader = Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.5));
    painter.line_segment([end, pos2(centre.x, at.y)], leader);
    text(painter, at, align, label);
}

/// An arc about `centre` of radius `r` from angle `from` to `to`, radians on screen.
pub(super) fn arc(painter: &Painter, centre: Pos2, r: f32, from: f32, to: f32) {
    let points: Vec<Pos2> = (0..=16)
        .map(|step| from + (to - from) * step as f32 / 16.0)
        .map(|angle| centre + Vec2::angled(angle) * r)
        .collect();
    painter.add(Shape::line(points, accent_line()));
}

/// The head of an arrow at `tip`, pointing away from `tail`.
pub(super) fn arrowhead(painter: &Painter, tip: Pos2, tail: Pos2) {
    let back = (tail - tip).normalized() * ARROW;
    if !back.is_finite() {
        return;
    }
    let side = back.rot90() * 0.6;
    let accent = theme::colors().accent;
    painter.add(Shape::convex_polygon(
        vec![tip, tip + back + side, tip + back - side],
        accent,
        Stroke::NONE,
    ));
}

/// A figure on a patch of the drawing's ground, so a line or a hatch under it does not
/// cross it.
pub(super) fn text(painter: &Painter, at: Pos2, align: Align2, label: &str) {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    let wanted = align.anchor_size(at, galley.size());
    let rect = free_spot(wanted, painter.clip_rect().shrink(2.0));
    painter.rect_filled(rect.expand2(vec2(2.0, 0.0)), 2.0, colors.sunken);
    painter.galley(rect.min, galley, colors.accent_soft);
}
