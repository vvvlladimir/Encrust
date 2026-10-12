//! The drawing over the Shape tool's fields: one support laid on its side from the model
//! to the plate, the part picked drawn larger and in the accent, with every measurement
//! its fields set written on it. It follows the numbers, so an edit shows as it is made;
//! widths are not to scale against lengths, or a tip half a millimetre across would not
//! show.

use egui::{Align2, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use printer_profiles::SupportProfile;

use crate::ui::theme;

/// The drawing's height, points; its width is the panel's.
const HEIGHT: f32 = 184.0;
/// Room between the drawing and its frame, and the width of a hatched wall, points.
const INSET: f32 = 12.0;
const WALL_W: f32 = 12.0;
const HATCH_GAP: f32 = 7.0;
/// Where the support's axis runs, as a share of the drawing's height.
const AXIS_AT: f32 = 0.42;

/// Points a millimetre of width is drawn at, and the least and most half a width may be:
/// the most keeps two rows of figures above the widest part.
const WIDTH_PT: f32 = 12.0;
const MIN_HALF: f32 = 2.0;
const MAX_HALF: f32 = 38.0;

/// Rows of figures off the widest part drawn, points: the first and the second.
const LANE: f32 = 11.0;
const LANE_STEP: f32 = 15.0;

const ARROW: f32 = 4.0;
const LINE_W: f32 = 1.2;

/// One part of a regular support, which is what the Part switch picks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Part {
    #[default]
    Tip,
    Body,
    Base,
    Raft,
    Brace,
}

impl Part {
    pub const ALL: [Self; 5] = [Self::Tip, Self::Body, Self::Base, Self::Raft, Self::Brace];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tip => "Tip",
            Self::Body => "Body",
            Self::Base => "Base",
            Self::Raft => "Raft",
            Self::Brace => "Brace",
        }
    }

    /// How much of the support's length this part is drawn over: the part picked is
    /// drawn larger, so its measurements have room.
    fn share(self, picked: Part) -> f32 {
        let large = self == picked;
        match self {
            Self::Tip if large => 0.36,
            Self::Tip => 0.18,
            Self::Base if large => 0.24,
            Self::Base => 0.12,
            Self::Raft if large => 0.12,
            Self::Raft => 0.05,
            Self::Body | Self::Brace => 0.0,
        }
    }
}

/// Half of `diameter_mm` as drawn.
fn half(diameter_mm: f32) -> f32 {
    (diameter_mm * WIDTH_PT / 2.0).clamp(MIN_HALF, MAX_HALF)
}

fn mm(value: f32) -> String {
    format!("{value:.2}")
}

fn diameter(value: f32) -> String {
    format!("Ø{value:.2}")
}

/// Where each part of a regular support stands in the drawing, left to right, and the
/// rows its figures are written in.
struct Layout {
    frame: Rect,
    axis: f32,
    /// The model's face, which the tip bites into, and the end of the bite inside it.
    contact: f32,
    bite: f32,
    neck: f32,
    flare: f32,
    pad: f32,
    raft: f32,
    plate: f32,
}

impl Layout {
    fn of(frame: Rect, profile: &SupportProfile, picked: Part) -> Self {
        let contact = frame.left() + INSET + WALL_W;
        let plate = frame.right() - INSET - WALL_W;
        let length = plate - contact;
        let raft_share = match profile.raft.enabled || picked == Part::Raft {
            true => Part::Raft.share(picked),
            false => 0.0,
        };
        let raft = plate - length * raft_share;
        let base = length * Part::Base.share(picked);
        let pad = raft - base * 0.4;
        let flare = raft - base;
        let neck = contact + length * Part::Tip.share(picked);
        let bite = (profile.tip.contact_depth_mm * WIDTH_PT * 2.0).clamp(3.0, WALL_W - 2.0);
        Self {
            frame,
            axis: frame.top() + frame.height() * AXIS_AT,
            contact,
            bite: contact - bite,
            neck,
            flare,
            pad,
            raft,
            plate,
        }
    }

    /// The `row`th row of figures above the axis, clear of `reach` either side of it;
    /// a negative row is below.
    fn lane(&self, reach: f32, row: i32) -> f32 {
        let off = reach + LANE + LANE_STEP * (row.abs() - 1) as f32;
        if row > 0 {
            self.axis - off
        } else {
            self.axis + off
        }
    }

    fn along_body(&self, share: f32) -> f32 {
        self.neck + (self.flare - self.neck) * share
    }
}

/// How one outline is drawn: the accent over a wash when it is the part picked, a plain
/// line otherwise, a faint one for what only sets the scene, and dashed when off.
#[derive(Clone, Copy, PartialEq)]
enum Pen {
    Picked,
    Plain,
    Faint,
}

impl Pen {
    fn of(part: Part, picked: Part) -> Self {
        if part == picked {
            Self::Picked
        } else {
            Self::Plain
        }
    }

    fn outline(self, points: Vec<Pos2>, off: bool) -> Vec<Shape> {
        let colors = theme::colors();
        let (colour, fill) = match self {
            Self::Picked => (colors.accent, colors.accent_wash),
            Self::Plain => (colors.text_low, egui::Color32::TRANSPARENT),
            Self::Faint => (colors.line, egui::Color32::TRANSPARENT),
        };
        let stroke = Stroke::new(LINE_W, colour);
        if off {
            let mut closed = points.clone();
            closed.extend(points.first().copied());
            return Shape::dashed_line(&closed, stroke, 4.0, 3.0);
        }
        vec![Shape::convex_polygon(points, fill, stroke)]
    }
}

/// Draws a regular support with `picked` larger and in the accent, its measurements on it.
pub fn regular(ui: &mut Ui, profile: &SupportProfile, picked: Part) {
    let frame = canvas(ui);
    let painter = ui.painter_at(frame);
    let at = Layout::of(frame, profile, picked);
    let (top, bottom) = (frame.top() + INSET, frame.bottom() - INSET);
    hatch(
        &painter,
        Rect::from_x_y_ranges(at.contact - WALL_W..=at.contact, top..=bottom),
    );
    hatch(
        &painter,
        Rect::from_x_y_ranges(at.plate..=at.plate + WALL_W, top..=bottom),
    );

    let pen = |part: Part| Pen::of(part, picked);
    let mut shapes = Vec::new();
    shapes.extend(pen(Part::Tip).outline(tip(&at, profile), false));
    shapes.extend(pen(Part::Body).outline(body(&at, profile), false));
    for outline in base(&at, profile) {
        shapes.extend(pen(Part::Base).outline(outline, false));
    }
    if profile.raft.enabled || picked == Part::Raft {
        shapes.extend(pen(Part::Raft).outline(raft(&at, profile), !profile.raft.enabled));
    }
    painter.extend(shapes);
    match picked {
        Part::Tip => tip_figures(&painter, &at, profile),
        Part::Body => body_figures(&painter, &at, profile),
        Part::Base => base_figures(&painter, &at, profile),
        Part::Raft => raft_figures(&painter, &at, profile),
        Part::Brace => brace_figures(&painter, &at, profile),
    }
}

/// The bite into the model and the cone widening out of it, as one outline.
fn tip(at: &Layout, profile: &SupportProfile) -> Vec<Pos2> {
    let contact = half(profile.tip.contact_diameter_mm);
    let pillar = half(profile.middle.diameter_mm);
    let y = at.axis;
    vec![
        pos2(at.bite, y - contact),
        pos2(at.contact, y - contact),
        pos2(at.neck, y - pillar),
        pos2(at.neck, y + pillar),
        pos2(at.contact, y + contact),
        pos2(at.bite, y + contact),
    ]
}

fn body(at: &Layout, profile: &SupportProfile) -> Vec<Pos2> {
    let pillar = half(profile.middle.diameter_mm);
    corners(Rect::from_x_y_ranges(
        at.neck..=at.flare,
        at.axis - pillar..=at.axis + pillar,
    ))
}

/// The flare out of the pillar and the pad it stands on.
fn base(at: &Layout, profile: &SupportProfile) -> [Vec<Pos2>; 2] {
    let y = at.axis;
    let (upper, lower, pad) = base_halves(profile);
    let flare = vec![
        pos2(at.flare, y - upper),
        pos2(at.pad, y - lower),
        pos2(at.pad, y + lower),
        pos2(at.flare, y + upper),
    ];
    let pad = Rect::from_x_y_ranges(at.pad..=at.raft, y - pad..=y + pad);
    [flare, corners(pad)]
}

/// The pillar's width where the flare leaves it, the flare's where it meets the pad and
/// the pad's, as drawn: none narrower than the one above it.
fn base_halves(profile: &SupportProfile) -> (f32, f32, f32) {
    let upper = half(profile.middle.diameter_mm);
    let lower = half(profile.bottom.lower_diameter_mm).max(upper);
    (
        upper,
        lower,
        half(profile.bottom.platform_diameter_mm).max(lower),
    )
}

/// The raft as a slab under the pads, its wall leaning out to the plate by its slope.
fn raft(at: &Layout, profile: &SupportProfile) -> Vec<Pos2> {
    let (outer, inner) = raft_halves(at, profile);
    let y = at.axis;
    vec![
        pos2(at.raft, y - inner),
        pos2(at.plate, y - outer),
        pos2(at.plate, y + outer),
        pos2(at.raft, y + inner),
    ]
}

/// How wide the raft is drawn where it meets the plate and where it meets the pads.
fn raft_halves(at: &Layout, profile: &SupportProfile) -> (f32, f32) {
    let (_, _, pad) = base_halves(profile);
    let outer = MAX_HALF.max(pad);
    let lean = (at.plate - at.raft) * profile.raft.slope_deg.to_radians().tan();
    (outer, (outer - lean).max(pad))
}

/// The contact, the bite and the cone's length.
fn tip_figures(painter: &egui::Painter, at: &Layout, profile: &SupportProfile) {
    let y = at.axis;
    let tip = &profile.tip;
    let top = &profile.top;
    let contact = half(tip.contact_diameter_mm);
    let reach = contact.max(half(profile.middle.diameter_mm));
    let length_y = at.lane(reach, 1);
    dimension(
        painter,
        pos2(at.contact, length_y),
        pos2(at.neck, length_y),
        &mm(top.length_mm),
    );
    let bite_y = at.lane(reach, -1);
    dimension(painter, pos2(at.bite, bite_y), pos2(at.contact, bite_y), "");
    text(
        painter,
        pos2(at.contact + 3.0, bite_y),
        Align2::LEFT_CENTER,
        &mm(tip.contact_depth_mm),
    );

    let stub = (at.bite + at.contact) / 2.0;
    let contact_label = pos2(stub, at.lane(reach, -2));
    across(
        painter,
        stub,
        y,
        contact,
        &diameter(tip.contact_diameter_mm),
        contact_label,
    );
}

/// The pillar's width, the air it keeps, and the branch that merges into it.
fn body_figures(painter: &egui::Painter, at: &Layout, profile: &SupportProfile) {
    let y = at.axis;
    let pillar = half(profile.middle.diameter_mm);
    let clear = (profile.clearance_mm * WIDTH_PT).clamp(2.0, 16.0);
    let air = Stroke::new(1.0, theme::colors().text_low);
    for edge in [y - pillar - clear, y + pillar + clear] {
        let line = [pos2(at.neck, edge), pos2(at.flare, edge)];
        painter.extend(Shape::dashed_line(&line, air, 4.0, 3.0));
    }
    let reach = pillar + clear;
    let pillar_x = at.along_body(0.12);
    let pillar_label = pos2(pillar_x, at.lane(reach, -1));
    across(
        painter,
        pillar_x,
        y,
        pillar,
        &diameter(profile.middle.diameter_mm),
        pillar_label,
    );
    let clear_x = at.along_body(0.3);
    let (from, to) = (pos2(clear_x, y + pillar), pos2(clear_x, y + pillar + clear));
    pointed(painter, from, to);
    text(
        painter,
        pos2(clear_x, at.lane(reach, -2)),
        Align2::CENTER_CENTER,
        &mm(profile.clearance_mm),
    );
    branch(painter, at, profile, reach);
}

/// A second tip merging into the pillar at the branch angle, the reach it merges within
/// and the widest the trunk may grow; a note when branching is off.
fn branch(painter: &egui::Painter, at: &Layout, profile: &SupportProfile, reach: f32) {
    let branching = &profile.branching;
    let merge = pos2(at.along_body(0.62), at.axis);
    if !branching.enabled {
        text(
            painter,
            pos2(merge.x, at.lane(reach, 1)),
            Align2::CENTER_CENTER,
            "no branching",
        );
        return;
    }
    let angle = branching.max_angle_deg.to_radians();
    let out = vec2(-angle.cos(), -angle.sin());
    // Short of the frame's top by the row its figure takes.
    let room = merge.y - at.frame.top() - INSET - LANE_STEP;
    let length = ((merge.x - at.neck) * 0.75).min(room / angle.sin().max(0.2));
    let end = merge + out * length;
    let pillar = half(profile.middle.diameter_mm);
    let across_out = out.rot90() * pillar;
    let tube = vec![
        merge - across_out,
        end - across_out,
        end + across_out,
        merge + across_out,
    ];
    painter.extend(Pen::Picked.outline(tube, false));
    arc(
        painter,
        merge,
        22.0,
        std::f32::consts::PI,
        std::f32::consts::PI + angle,
    );
    let mid = std::f32::consts::PI + angle / 2.0;
    let arc_label = merge + vec2(mid.cos(), mid.sin()) * 40.0;
    text(
        painter,
        arc_label,
        Align2::CENTER_CENTER,
        &format!("{:.0}°", branching.max_angle_deg),
    );
    let merge_label = format!("≤{:.1}", branching.max_merge_distance_mm);
    text(
        painter,
        end + vec2(0.0, -10.0),
        Align2::CENTER_BOTTOM,
        &merge_label,
    );

    let trunk = half(branching.max_trunk_diameter_mm).max(pillar);
    let trunk_x = at.along_body(0.85);
    let trunk_label = pos2(trunk_x, at.lane(reach.max(trunk), -1));
    across(
        painter,
        trunk_x,
        at.axis,
        trunk,
        &format!("Ø≤{:.2}", branching.max_trunk_diameter_mm),
        trunk_label,
    );
}

/// The flare's width at the pad, the pad's width and its thickness.
fn base_figures(painter: &egui::Painter, at: &Layout, profile: &SupportProfile) {
    let y = at.axis;
    let bottom = &profile.bottom;
    let (_, lower, pad) = base_halves(profile);
    let low_x = at.pad - 3.0;
    let low_label = pos2(low_x - 8.0, at.lane(pad, -2));
    across(
        painter,
        low_x,
        y,
        lower,
        &diameter(bottom.lower_diameter_mm),
        low_label,
    );
    let pad_x = (at.pad + at.raft) / 2.0;
    let pad_label = pos2(pad_x, at.lane(pad, 1));
    across(
        painter,
        pad_x,
        y,
        pad,
        &diameter(bottom.platform_diameter_mm),
        pad_label,
    );
    let thick_y = at.lane(pad, -1);
    dimension(painter, pos2(at.pad, thick_y), pos2(at.raft, thick_y), "");
    text(
        painter,
        pos2(at.raft + 3.0, thick_y),
        Align2::LEFT_CENTER,
        &mm(bottom.platform_thickness_mm),
    );
}

/// The slab's thickness, how much wider than the feet it is, its wall's slope, and how far
/// the whole support lifts the model off the plate.
fn raft_figures(painter: &egui::Painter, at: &Layout, profile: &SupportProfile) {
    let raft = &profile.raft;
    let (outer, inner) = raft_halves(at, profile);
    let lift_y = at.lane(outer, -1);
    dimension(
        painter,
        pos2(at.contact, lift_y),
        pos2(at.plate, lift_y),
        &format!("lift {}", mm(profile.lift_mm())),
    );
    let top_y = at.lane(outer, 1);
    if !raft.enabled {
        text(
            painter,
            pos2(at.raft, top_y),
            Align2::CENTER_CENTER,
            "no raft",
        );
        return;
    }
    dimension(painter, pos2(at.raft, top_y), pos2(at.plate, top_y), "");
    text(
        painter,
        pos2(at.raft - 3.0, top_y),
        Align2::RIGHT_CENTER,
        &mm(raft.thickness_mm),
    );
    let corner = pos2(at.raft, at.axis - inner);
    let lean = (pos2(at.plate, at.axis - outer) - corner).angle();
    arc(painter, corner, 16.0, -std::f32::consts::FRAC_PI_2, lean);
    let slope = format!("{:.0}°", raft.slope_deg);
    text(
        painter,
        corner + vec2(-6.0, -12.0),
        Align2::RIGHT_CENTER,
        &slope,
    );
    let area = format!("×{:.2}", raft.area_ratio);
    text(
        painter,
        pos2((at.raft + at.plate) / 2.0, at.axis),
        Align2::CENTER_CENTER,
        &area,
    );
}

/// A neighbouring trunk below, the braces tying it to this one, and where they go.
fn brace_figures(painter: &egui::Painter, at: &Layout, profile: &SupportProfile) {
    let bracing = &profile.bracing;
    let pillar = half(profile.middle.diameter_mm);
    let other_y = at.frame.bottom() - INSET - pillar - 18.0;
    let other = Rect::from_x_y_ranges(
        at.neck + 20.0..=at.flare,
        other_y - pillar..=other_y + pillar,
    );
    painter.extend(Pen::Faint.outline(corners(other), false));
    let (first, step) = (at.along_body(0.88), (at.flare - at.neck) * 0.3);
    let rise = other_y - at.axis - 2.0 * pillar;
    let braces = [first, first - step].map(|low_x| {
        let low = pos2(low_x, other_y - pillar);
        (low, low + vec2(-rise, -rise))
    });
    let thick = half(bracing.diameter_mm).min(3.0);
    for (low, high) in braces {
        let side = (high - low).normalized().rot90() * thick;
        painter.extend(Pen::Picked.outline(
            vec![low - side, high - side, high + side, low + side],
            !bracing.enabled,
        ));
    }
    let (low, high) = braces[0];
    text(
        painter,
        low.lerp(high, 0.5) + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        &diameter(bracing.diameter_mm),
    );
    let below = other_y + pillar + 16.0;
    dimension(
        painter,
        pos2(low.x, below),
        pos2(at.plate, below),
        &format!("first {}", mm(bracing.start_height_mm)),
    );
    let over = at.lane(pillar, 1);
    dimension(
        painter,
        pos2(braces[1].1.x, over),
        pos2(high.x, over),
        &format!("every {}", mm(bracing.rise_mm)),
    );
    let span_x = at.flare - 8.0;
    dimension(
        painter,
        pos2(span_x, at.axis),
        pos2(span_x, other_y),
        &format!("≤{:.1}", bracing.max_spacing_mm),
    );
    if !bracing.enabled {
        text(
            painter,
            pos2(at.along_body(0.25), (at.axis + other_y) / 2.0),
            Align2::CENTER_CENTER,
            "no bracing",
        );
    }
}

/// Draws the thin strut between two parts of the model, both ends sunk into a wall.
pub fn small_pillar(ui: &mut Ui, profile: &SupportProfile) {
    let frame = canvas(ui);
    let painter = ui.painter_at(frame);
    let (top, bottom) = (frame.top() + INSET, frame.bottom() - INSET);
    let left = frame.left() + INSET + WALL_W;
    let right = frame.right() - INSET - WALL_W;
    hatch(
        &painter,
        Rect::from_x_y_ranges(left - WALL_W..=left, top..=bottom),
    );
    hatch(
        &painter,
        Rect::from_x_y_ranges(right..=right + WALL_W, top..=bottom),
    );

    let pillar = &profile.small_pillar;
    let y = frame.top() + frame.height() * AXIS_AT;
    let thick = half(pillar.diameter_mm);
    let sunk = |depth_mm: f32| (depth_mm * WIDTH_PT * 2.0).clamp(3.0, WALL_W - 2.0);
    let (upper, lower) = (sunk(pillar.upper_depth_mm), sunk(pillar.lower_depth_mm));
    let rect = Rect::from_x_y_ranges(left - upper..=right + lower, y - thick..=y + thick);
    painter.extend(Pen::Picked.outline(corners(rect), !pillar.enabled));

    let middle = (left + right) / 2.0;
    let label = pos2(middle, y - thick - LANE - 4.0);
    across(
        &painter,
        middle,
        y,
        thick,
        &diameter(pillar.diameter_mm),
        label,
    );
    // A depth is a few points across, so its figure stands beside it, off the wall.
    let under = y + thick + LANE;
    dimension(&painter, pos2(left - upper, under), pos2(left, under), "");
    dimension(&painter, pos2(right, under), pos2(right + lower, under), "");
    text(
        &painter,
        pos2(left + 4.0, under),
        Align2::LEFT_CENTER,
        &mm(pillar.upper_depth_mm),
    );
    text(
        &painter,
        pos2(right - 4.0, under),
        Align2::RIGHT_CENTER,
        &mm(pillar.lower_depth_mm),
    );
}

/// Takes the drawing's room out of the panel and frames it.
fn canvas(ui: &mut Ui) -> Rect {
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

fn corners(rect: Rect) -> Vec<Pos2> {
    vec![
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

/// A wall the drawing stands against, as a draughtsman hatches a section.
fn hatch(painter: &egui::Painter, rect: Rect) {
    let colors = theme::colors();
    let clipped = painter.with_clip_rect(rect);
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

fn accent_line() -> Stroke {
    Stroke::new(1.0, theme::colors().accent)
}

/// A measured length between `from` and `to`, arrowed at both ends, its figure over its
/// middle when it has one.
fn dimension(painter: &egui::Painter, from: Pos2, to: Pos2, label: &str) {
    painter.line_segment([from, to], accent_line());
    arrowhead(painter, from, to);
    arrowhead(painter, to, from);
    if label.is_empty() {
        return;
    }
    let middle = from.lerp(to, 0.5);
    let length = (to - from).length();
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

fn label_width(painter: &egui::Painter, label: &str) -> f32 {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    galley.size().x
}

/// An arrow from `from` to `to` with its head at `to`.
fn pointed(painter: &egui::Painter, from: Pos2, to: Pos2) {
    painter.line_segment([from, to], accent_line());
    arrowhead(painter, to, from);
}

/// A diameter: the arrow across `half` either side of `y` at `x`, its figure centred on
/// `label_at`. One too narrow for its own arrowheads is pointed at from outside.
fn across(painter: &egui::Painter, x: f32, y: f32, half: f32, label: &str, label_at: Pos2) {
    let (from, to) = (pos2(x, y - half), pos2(x, y + half));
    if half * 2.0 < ARROW * 3.0 {
        let reach = vec2(0.0, ARROW * 2.5);
        pointed(painter, from - reach, from);
        pointed(painter, to + reach, to);
    } else {
        dimension(painter, from, to, "");
    }
    // The leader from the figure to the arrow, when the figure stands off it.
    let end = if label_at.y < y { from } else { to };
    if (label_at.y - end.y).abs() > LANE {
        let leader = Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.5));
        painter.line_segment([pos2(x, end.y), pos2(label_at.x, label_at.y)], leader);
    }
    text(painter, label_at, Align2::CENTER_CENTER, label);
}

/// An arc about `centre` of radius `r` from angle `from` to `to`, radians on screen.
fn arc(painter: &egui::Painter, centre: Pos2, r: f32, from: f32, to: f32) {
    let points: Vec<Pos2> = (0..=16)
        .map(|step| from + (to - from) * step as f32 / 16.0)
        .map(|angle| centre + Vec2::angled(angle) * r)
        .collect();
    painter.add(Shape::line(points, accent_line()));
}

/// The head of an arrow at `tip`, pointing away from `tail`.
fn arrowhead(painter: &egui::Painter, tip: Pos2, tail: Pos2) {
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
fn text(painter: &egui::Painter, at: Pos2, align: Align2, label: &str) {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand2(vec2(2.0, 0.0)), 2.0, colors.sunken);
    painter.galley(rect.min, galley, colors.accent_soft);
}
