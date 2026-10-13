//! The drawing over the Shape tool's fields: one support laid on its side from the model
//! to the plate, the part picked in the accent, with every measurement its fields set
//! written on it. It follows the numbers, so an edit shows as it is made. Widths share one
//! scale and keep their proportions to each other; lengths do not, or a tip half a
//! millimetre across would not show.

use egui::{Align2, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use printer_profiles::SupportProfile;

use crate::ui::theme;

/// The drawing's height, points; its width is the panel's.
const HEIGHT: f32 = 196.0;
/// Room between the drawing and its frame, and the width of a hatched wall, points.
const INSET: f32 = 12.0;
const WALL_W: f32 = 12.0;
const HATCH_GAP: f32 = 7.0;
/// Room over the walls for the word saying what each one is, points.
const CAPTION: f32 = 14.0;

/// Points a millimetre of width is drawn at, and the least and most half a width may be:
/// the most keeps two rows of figures above the widest part.
const WIDTH_PT: f32 = 24.0;
const MIN_HALF: f32 = 1.5;
const MAX_HALF: f32 = 36.0;

/// Rows of figures off the widest part drawn, points: the first and the second.
const LANE: f32 = 11.0;
const LANE_STEP: f32 = 15.0;
/// How tall a figure stands, and the least width one is written inside, points.
const FIGURE_H: f32 = 14.0;
const FITS: f32 = 16.0;
/// How far apart the axes of the two trunks a brace ties are drawn, points.
const BRACE_GAP: f32 = 64.0;

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
            Self::Raft if large => 0.16,
            Self::Raft => 0.06,
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
    /// The highest a figure may stand, and the support's axis.
    top: f32,
    axis: f32,
    /// The model's face, which the tip bites into, and the end of the bite inside it.
    contact: f32,
    bite: f32,
    neck: f32,
    /// Where the pillar widens into the trunk a branch merges into; the flare when none does.
    merge: f32,
    flare: f32,
    /// Where the flare lands: the pad's face, or the raft's where the raft stands in for it.
    foot: f32,
    plate: f32,
    /// Whether the raft is drawn, in the pad's place.
    raft: bool,
    /// Half the pillar's width, and the trunk's below the merge.
    pillar: f32,
    trunk: f32,
}

impl Layout {
    fn of(frame: Rect, profile: &SupportProfile, picked: Part) -> Self {
        let contact = frame.left() + INSET + WALL_W;
        let plate = frame.right() - INSET - WALL_W;
        let length = plate - contact;
        let raft = picked == Part::Raft || (profile.raft.enabled && picked != Part::Base);
        let base = length * Part::Base.share(picked);
        let foot = match raft {
            true => plate - length * Part::Raft.share(picked),
            false => plate - base * 0.4,
        };
        let pillar = half(profile.middle.diameter_mm);
        let branched = picked == Part::Body && profile.branching.enabled;
        let radius = profile.middle.diameter_mm / 2.0;
        let trunk = match branched {
            true => half(2.0 * profile.branching.merged_radius_mm(radius, radius)).max(pillar),
            false => pillar,
        };
        let bite = (profile.tip.contact_depth_mm * WIDTH_PT).clamp(3.0, WALL_W - 2.0);
        let shift = if picked == Part::Brace {
            BRACE_GAP * 0.375
        } else {
            0.0
        };
        let mut at = Self {
            top: frame.top() + INSET,
            axis: frame.center().y - shift,
            contact,
            bite: contact - bite,
            neck: contact + length * Part::Tip.share(picked),
            merge: foot - base * 0.6,
            flare: foot - base * 0.6,
            foot,
            plate,
            raft,
            pillar,
            trunk,
        };
        if branched {
            at.merge = Branch::of(&at, profile).merge.x;
        }
        at
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
}

/// A second tip's branch: where it leaves the model beside the first tip, and where it
/// merges into the pillar at the steepest lean the profile allows.
struct Branch {
    start: Pos2,
    merge: Pos2,
}

impl Branch {
    fn of(at: &Layout, profile: &SupportProfile) -> Self {
        let across_per_along = profile.branching.reach_per_drop();
        let merge = at.neck + (at.flare - at.neck) * 0.55;
        // Under the walls' captions with a row for the angle, and clear of the first cone.
        let room = at.axis - at.top - CAPTION - FIGURE_H;
        let side = room.min((merge - at.neck - 8.0) * across_per_along);
        Self {
            start: pos2(merge - side / across_per_along, at.axis - side),
            merge: pos2(merge, at.axis),
        }
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
    walls(&painter, frame, ["model", "plate"]);

    let pen = |part: Part| Pen::of(part, picked);
    let mut shapes = Vec::new();
    shapes.extend(pen(Part::Tip).outline(tip(&at, profile), false));
    for outline in body(&at) {
        shapes.extend(pen(Part::Body).outline(outline, false));
    }
    shapes.extend(pen(Part::Base).outline(flare(&at, profile), false));
    if at.raft {
        shapes.extend(pen(Part::Raft).outline(raft(&at, profile), !profile.raft.enabled));
    } else {
        shapes.extend(pen(Part::Base).outline(corners(pad(&at, profile)), false));
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
    let y = at.axis;
    vec![
        pos2(at.bite, y - contact),
        pos2(at.contact, y - contact),
        pos2(at.neck, y - at.pillar),
        pos2(at.neck, y + at.pillar),
        pos2(at.contact, y + contact),
        pos2(at.bite, y + contact),
    ]
}

/// The pillar, and the wider trunk it becomes below a merge.
fn body(at: &Layout) -> Vec<Vec<Pos2>> {
    let band = |from: f32, to: f32, half: f32| {
        corners(Rect::from_x_y_ranges(
            from..=to,
            at.axis - half..=at.axis + half,
        ))
    };
    let mut bands = vec![band(at.neck, at.merge, at.pillar)];
    if at.merge < at.flare {
        bands.push(band(at.merge, at.flare, at.trunk));
    }
    bands
}

/// The flare out of the body down to the foot.
fn flare(at: &Layout, profile: &SupportProfile) -> Vec<Pos2> {
    let (lower, _) = foot_halves(at, profile);
    let y = at.axis;
    vec![
        pos2(at.flare, y - at.trunk),
        pos2(at.foot, y - lower),
        pos2(at.foot, y + lower),
        pos2(at.flare, y + at.trunk),
    ]
}

/// The flare's width where it lands and the pad's, as drawn: neither narrower than the
/// body above it.
fn foot_halves(at: &Layout, profile: &SupportProfile) -> (f32, f32) {
    let lower = half(profile.bottom.lower_diameter_mm).max(at.trunk);
    (lower, half(profile.bottom.platform_diameter_mm).max(lower))
}

fn pad(at: &Layout, profile: &SupportProfile) -> Rect {
    let (_, pad) = foot_halves(at, profile);
    Rect::from_x_y_ranges(at.foot..=at.plate, at.axis - pad..=at.axis + pad)
}

/// The raft in the pad's place: narrow where it meets the plate, so a blade gets under
/// it, its walls leaning out by its slope towards the supports.
fn raft(at: &Layout, profile: &SupportProfile) -> Vec<Pos2> {
    let (upper, lower) = raft_halves(at, profile);
    let y = at.axis;
    vec![
        pos2(at.foot, y - upper),
        pos2(at.plate, y - lower),
        pos2(at.plate, y + lower),
        pos2(at.foot, y + upper),
    ]
}

/// How wide the raft is drawn where the supports stand on it and where it meets the
/// plate, short of the row its thickness is written in.
fn raft_halves(at: &Layout, profile: &SupportProfile) -> (f32, f32) {
    let (_, pad) = foot_halves(at, profile);
    let lower = pad + 4.0;
    let lean = profile.raft.slope_deg.clamp(0.0, 85.0).to_radians().tan();
    let room = at.axis - at.top - CAPTION - LANE - FIGURE_H / 2.0;
    let upper = (lower + (at.plate - at.foot) * lean).min(room);
    (upper.max(lower), lower)
}

/// The contact, the bite and the cone's length.
fn tip_figures(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    let tip = &profile.tip;
    let contact = half(tip.contact_diameter_mm);
    let reach = contact.max(at.pillar);
    let x = at.contact + 4.0;
    let above = (pos2(x - 4.0, at.lane(reach, 1)), Align2::LEFT_CENTER);
    across(
        painter,
        pos2(x, at.axis),
        contact,
        &diameter(tip.contact_diameter_mm),
        None,
        above,
    );
    let length_y = at.lane(reach, 2);
    dimension(
        painter,
        pos2(at.contact, length_y),
        pos2(at.neck, length_y),
        &mm(profile.top.length_mm),
    );
    let bite_y = at.lane(reach, -1);
    dimension(painter, pos2(at.bite, bite_y), pos2(at.contact, bite_y), "");
    text(
        painter,
        pos2(at.contact + ARROW * 3.0, bite_y),
        Align2::LEFT_CENTER,
        &mm(tip.contact_depth_mm),
    );
}

/// The pillar's width and the air it keeps, and the branch that merges into it.
fn body_figures(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    branch(painter, at, profile);
    let clear = clearance(painter, at, profile);
    let y = at.axis;
    let pillar = diameter(profile.middle.diameter_mm);
    if at.merge >= at.flare {
        let x = at.neck + 14.0;
        let below = (
            pos2(x - 4.0, at.lane(at.pillar + clear, -1)),
            Align2::LEFT_CENTER,
        );
        across(
            painter,
            pos2(x, y),
            at.pillar,
            &pillar,
            Some(Align2::LEFT_CENTER),
            below,
        );
        return;
    }
    let x = at.neck + 10.0;
    let below = (
        pos2(x + 4.0, at.lane(at.pillar + clear, -1)),
        Align2::RIGHT_CENTER,
    );
    across(painter, pos2(x, y), at.pillar, &pillar, None, below);
    let x = at.flare - 64.0;
    let below = (
        pos2(x, at.lane(at.trunk + clear, -1)),
        Align2::CENTER_CENTER,
    );
    let widest = format!("Ø≤{:.2}", profile.branching.max_trunk_diameter_mm);
    across(
        painter,
        pos2(x, y),
        at.trunk,
        &widest,
        Some(Align2::LEFT_CENTER),
        below,
    );
}

/// The air the body keeps, dashed along its underside, its width written in the gap.
/// Returns the gap as drawn.
fn clearance(painter: &Painter, at: &Layout, profile: &SupportProfile) -> f32 {
    let clear = (profile.clearance_mm * WIDTH_PT).clamp(6.0, 20.0);
    let y = at.axis;
    let line = [
        pos2(at.neck, y + at.pillar + clear),
        pos2(at.merge, y + at.pillar + clear),
        pos2(at.merge, y + at.trunk + clear),
        pos2(at.flare, y + at.trunk + clear),
    ];
    let air = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(&line, air, 4.0, 3.0));
    let x = at.flare - 36.0;
    let (from, to) = (pos2(x, y + at.trunk), pos2(x, y + at.trunk + clear));
    dimension(painter, from, to, "");
    text(
        painter,
        pos2(x + 4.0, from.lerp(to, 0.5).y),
        Align2::LEFT_CENTER,
        &mm(profile.clearance_mm),
    );
    clear
}

/// A second tip beside the first merging into the pillar at the branch angle, and how
/// far apart the two may be; a note when branching is off.
fn branch(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    let branching = &profile.branching;
    if !branching.enabled {
        let over = pos2((at.neck + at.flare) / 2.0, at.lane(at.pillar, 1));
        text(painter, over, Align2::CENTER_CENTER, "no branching");
        return;
    }
    let Branch { start, merge } = Branch::of(at, profile);
    let side = (merge - start).normalized().rot90();
    let contact = half(profile.tip.contact_diameter_mm);
    let arm = vec![
        start - side * contact,
        merge - side * at.pillar,
        merge + side * at.pillar,
        start + side * contact,
    ];
    painter.extend(Pen::Picked.outline(arm, false));

    // One line along the axis through the second tip serves the spacing and, against the
    // branch carried on past its tip, the angle, which then stands clear of the branch.
    let span_x = start.x - 14.0;
    let lean = (merge - start).angle();
    let back = start - Vec2::angled(lean) * 30.0;
    let guide = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(
        &[pos2(span_x - 4.0, start.y), start],
        guide,
        4.0,
        3.0,
    ));
    painter.extend(Shape::dashed_line(&[start, back], guide, 4.0, 3.0));
    let left = -std::f32::consts::PI;
    arc(painter, start, 22.0, left, left + lean);
    let label_at = start + Vec2::angled(left + lean / 2.0) * 38.0 - vec2(0.0, 3.0);
    let angle = format!("{:.0}°", branching.max_angle_deg);
    text(painter, label_at, Align2::CENTER_CENTER, &angle);
    dimension(painter, pos2(span_x, start.y), pos2(span_x, at.axis), "");
    let spacing = format!("≤{:.1}", branching.max_merge_distance_mm);
    let beside = pos2(span_x - 4.0, (start.y + at.axis - at.pillar) / 2.0);
    text(painter, beside, Align2::RIGHT_CENTER, &spacing);
}

/// The flare's width at the foot, the pad's width and its thickness.
fn base_figures(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    let y = at.axis;
    let bottom = &profile.bottom;
    let (lower, pad) = foot_halves(at, profile);
    let low_x = at.foot - 4.0;
    let below = (pos2(low_x, at.lane(lower, -1)), Align2::RIGHT_CENTER);
    let low = diameter(bottom.lower_diameter_mm);
    across(painter, pos2(low_x, y), lower, &low, None, below);
    let pad_x = (at.foot + at.plate) / 2.0;
    let above = (pos2(pad_x + 6.0, at.lane(pad, 1)), Align2::RIGHT_CENTER);
    let width = diameter(bottom.platform_diameter_mm);
    across(painter, pos2(pad_x, y), pad, &width, None, above);
    let thick_y = at.lane(pad, -2);
    dimension(painter, pos2(at.foot, thick_y), pos2(at.plate, thick_y), "");
    text(
        painter,
        pos2(at.foot - 4.0, thick_y),
        Align2::RIGHT_CENTER,
        &mm(bottom.platform_thickness_mm),
    );
}

/// The pad the raft stands in for, the slab's thickness, how far it reaches past the
/// pad and the lean of its wall.
fn raft_figures(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    let raft = &profile.raft;
    let (upper, lower) = raft_halves(at, profile);
    painter.extend(Pen::Faint.outline(corners(pad(at, profile)), false));
    let top_y = at.lane(upper, 1);
    if !raft.enabled {
        text(
            painter,
            pos2(at.foot, top_y),
            Align2::RIGHT_CENTER,
            "no raft",
        );
        return;
    }
    dimension(painter, pos2(at.foot, top_y), pos2(at.plate, top_y), "");
    text(
        painter,
        pos2(at.foot - 4.0, top_y),
        Align2::RIGHT_CENTER,
        &mm(raft.thickness_mm),
    );
    let (_, pad) = foot_halves(at, profile);
    let x = at.foot + 5.0;
    let edge = upper - (upper - lower) * 5.0 / (at.plate - at.foot);
    dimension(painter, pos2(x, at.axis - edge), pos2(x, at.axis - pad), "");
    text(
        painter,
        pos2(at.foot - 4.0, at.axis - (edge + pad) / 2.0),
        Align2::RIGHT_CENTER,
        &format!("+{}", mm(raft.margin_mm)),
    );
    // Off the plate's normal, at the corner where the wall leaves the supports' face.
    let corner = pos2(at.foot, at.axis + upper);
    let wall = pos2(at.plate, at.axis + lower) - corner;
    let r = (at.plate - at.foot) * 0.6;
    let normal = [corner, corner + vec2(r + 4.0, 0.0)];
    let guide = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(&normal, guide, 3.0, 2.0));
    arc(painter, corner, r, wall.angle(), 0.0);
    let slope = format!("{:.0}°", raft.slope_deg);
    text(
        painter,
        corner + vec2(r / 2.0, 4.0),
        Align2::CENTER_TOP,
        &slope,
    );
}

/// A neighbouring trunk below, the crossed braces tying it to this one, and where they go.
fn brace_figures(painter: &Painter, at: &Layout, profile: &SupportProfile) {
    let bracing = &profile.bracing;
    let pillar = at.pillar;
    let other_y = at.axis + BRACE_GAP;
    let other = Rect::from_x_y_ranges(
        at.neck + 20.0..=at.flare,
        other_y - pillar..=other_y + pillar,
    );
    painter.extend(Pen::Plain.outline(corners(other), false));
    let (high, low) = (at.axis + pillar, other_y - pillar);
    let rise = (at.flare - at.neck) * 0.26;
    let first = at.flare - 30.0;
    let thick = half(bracing.diameter_mm).min(pillar * 0.3);
    for level in [0.0, 1.0] {
        let (near, far) = (first - rise * level, first - rise * (level + 1.0));
        strut(
            painter,
            pos2(near, low),
            pos2(far, high),
            thick,
            !bracing.enabled,
        );
        strut(
            painter,
            pos2(near, high),
            pos2(far, low),
            thick,
            !bracing.enabled,
        );
    }

    let middle = at.axis + BRACE_GAP / 2.0;
    let left = first - rise * 2.0 - 6.0;
    text(
        painter,
        pos2(left, middle),
        Align2::RIGHT_CENTER,
        &diameter(bracing.diameter_mm),
    );
    let span_x = first + 8.0;
    dimension(painter, pos2(span_x, at.axis), pos2(span_x, other_y), "");
    let spacing = format!("≤{:.1}", bracing.max_spacing_mm);
    text(
        painter,
        pos2(span_x + 4.0, middle),
        Align2::LEFT_CENTER,
        &spacing,
    );
    let over = at.lane(pillar, 1);
    let every = format!("every {}", mm(bracing.rise_mm));
    dimension(painter, pos2(first - rise, over), pos2(first, over), &every);
    let under = other_y + pillar + LANE;
    dimension(painter, pos2(first, under), pos2(at.plate, under), "");
    let start = format!("first {}", mm(bracing.start_height_mm));
    let mid = (first + at.plate) / 2.0;
    text(painter, pos2(mid, under + 3.0), Align2::CENTER_TOP, &start);
    if !bracing.enabled {
        let note = pos2(first - rise, middle - FIGURE_H);
        text(painter, note, Align2::CENTER_BOTTOM, "no bracing");
    }
}

/// One brace from `from` to `to`, `thick` either side of its line.
fn strut(painter: &Painter, from: Pos2, to: Pos2, thick: f32, off: bool) {
    let side = (to - from).normalized().rot90() * thick;
    let outline = vec![from - side, to - side, to + side, from + side];
    painter.extend(Pen::Picked.outline(outline, off));
}

/// Draws the thin strut between two parts of the model, both ends sunk into a wall.
pub fn small_pillar(ui: &mut Ui, profile: &SupportProfile) {
    let frame = canvas(ui);
    let painter = ui.painter_at(frame);
    walls(&painter, frame, ["model", "model"]);
    let left = frame.left() + INSET + WALL_W;
    let right = frame.right() - INSET - WALL_W;

    let pillar = &profile.small_pillar;
    let y = frame.center().y;
    let thick = half(pillar.diameter_mm);
    let sunk = |depth_mm: f32| (depth_mm * WIDTH_PT).clamp(3.0, WALL_W - 2.0);
    let (upper, lower) = (sunk(pillar.upper_depth_mm), sunk(pillar.lower_depth_mm));
    let rect = Rect::from_x_y_ranges(left - upper..=right + lower, y - thick..=y + thick);
    painter.extend(Pen::Picked.outline(corners(rect), !pillar.enabled));

    let middle = (left + right) / 2.0;
    let above = (pos2(middle, y - thick - LANE - 4.0), Align2::CENTER_CENTER);
    let width = diameter(pillar.diameter_mm);
    across(&painter, pos2(middle, y), thick, &width, None, above);
    // A depth is a few points across, so its figure stands beside it, off the wall.
    let under = y + thick + LANE;
    dimension(&painter, pos2(left - upper, under), pos2(left, under), "");
    dimension(&painter, pos2(right, under), pos2(right + lower, under), "");
    text(
        &painter,
        pos2(left + ARROW * 3.0, under),
        Align2::LEFT_CENTER,
        &mm(pillar.upper_depth_mm),
    );
    text(
        &painter,
        pos2(right - ARROW * 3.0, under),
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

/// The two walls the drawing stands between, each with what it is written over it.
fn walls(painter: &Painter, frame: Rect, captions: [&str; 2]) {
    let (top, bottom) = (frame.top() + INSET, frame.bottom() - INSET);
    let (left, right) = (frame.left() + INSET, frame.right() - INSET);
    let span = top + CAPTION..=bottom;
    hatch(
        painter,
        Rect::from_x_y_ranges(left..=left + WALL_W, span.clone()),
    );
    hatch(painter, Rect::from_x_y_ranges(right - WALL_W..=right, span));
    let (font, colour) = (theme::figures(10.0), theme::colors().text_low);
    painter.text(
        pos2(left, top),
        Align2::LEFT_TOP,
        captions[0],
        font.clone(),
        colour,
    );
    painter.text(
        pos2(right, top),
        Align2::RIGHT_TOP,
        captions[1],
        font,
        colour,
    );
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
fn hatch(painter: &Painter, rect: Rect) {
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
fn dimension(painter: &Painter, from: Pos2, to: Pos2, label: &str) {
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

fn label_width(painter: &Painter, label: &str) -> f32 {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    galley.size().x
}

/// An arrow from `from` to `to` with its head at `to`.
fn pointed(painter: &Painter, from: Pos2, to: Pos2) {
    painter.line_segment([from, to], accent_line());
    arrowhead(painter, to, from);
}

/// A diameter: the arrow across `half` either side of `centre`. Its figure stands beside the arrow, aligned by
/// `inside`, when the width fits it; otherwise at `outside`, joined by a leader.
fn across(
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
fn arc(painter: &Painter, centre: Pos2, r: f32, from: f32, to: f32) {
    let points: Vec<Pos2> = (0..=16)
        .map(|step| from + (to - from) * step as f32 / 16.0)
        .map(|angle| centre + Vec2::angled(angle) * r)
        .collect();
    painter.add(Shape::line(points, accent_line()));
}

/// The head of an arrow at `tip`, pointing away from `tail`.
fn arrowhead(painter: &Painter, tip: Pos2, tail: Pos2) {
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
fn text(painter: &Painter, at: Pos2, align: Align2, label: &str) {
    let colors = theme::colors();
    let galley = painter.layout_no_wrap(label.to_owned(), theme::figures(10.5), colors.accent_soft);
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand2(vec2(2.0, 0.0)), 2.0, colors.sunken);
    painter.galley(rect.min, galley, colors.accent_soft);
}
