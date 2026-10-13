//! The drawing over the Shape tool's fields: one support laid on its side from the model
//! to the plate, the part picked in the accent, with every measurement its fields set
//! written on it. Each part is drawn true to scale, lengths and widths alike, from the
//! measurements the mesher builds it to, and zoomed to fill the frame; a long stretch
//! that carries nothing is cut out and marked as a break. See docs/design/supports.md.

use std::cell::RefCell;

use egui::{Align2, Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use printer_profiles::{ContactShape, PlatformShape, RaftStyle, SupportProfile};

use crate::ui::theme;

/// The drawing's height, points; its width is the panel's.
const HEIGHT: f32 = 196.0;
/// Room between the drawing and its frame, and the least width of a hatched wall, points.
const INSET: f32 = 12.0;
const WALL_W: f32 = 12.0;
const HATCH_GAP: f32 = 7.0;
/// Room over the walls for the word saying what each one is, points.
const CAPTION: f32 = 14.0;
/// Room kept either side of the drawing for two rows of figures, points.
const MARGIN: f32 = 34.0;
/// How wide a cut-out stretch is drawn, points.
const BREAK_W: f32 = 14.0;

/// Rows of figures off the widest part drawn, points: the first and the second.
const LANE: f32 = 11.0;
const LANE_STEP: f32 = 15.0;
/// How tall a figure stands, and the least width one is written inside, points.
const FIGURE_H: f32 = 14.0;
const FITS: f32 = 16.0;

const ARROW: f32 = 4.0;
const LINE_W: f32 = 1.2;

/// Room inside the model left of the tip for the contact's figure, points.
const TIP_LABEL_W: f32 = 44.0;

/// Room left of the braces for their figures, points.
const BRACE_LABEL_W: f32 = 52.0;

/// Rings a sphere tip is drawn with, and brace levels drawn between two trunks.
const BALL_STEPS: u32 = 12;
const BRACE_LEVELS: u32 = 2;

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
}

fn mm(value: f32) -> String {
    format!("{value:.2}")
}

fn diameter(value: f32) -> String {
    format!("Ø{value:.2}")
}

/// A side view at one scale: millimetres along the support from the model's face (x) and
/// across it from its axis (y), to points. Laid on its side the support runs left to
/// right; standing, it runs down to the plate. A cut takes a stretch out and draws it
/// `BREAK_W` wide.
struct View {
    frame: Rect,
    origin: Pos2,
    scale: f32,
    cuts: Vec<(f32, f32)>,
    standing: bool,
}

impl View {
    /// The scale that fits `along` lengthwise, less its cuts, and `half_mm` either side of
    /// the axis, with the start of `along` at the left of the frame. Where the height is
    /// what limits it, the first cut gives back what the width has to spare.
    fn fit(frame: Rect, along: (f32, f32), half_mm: f32, cuts: Vec<(f32, f32)>) -> Self {
        let room_h = frame.height() - 2.0 * (INSET + MARGIN) - CAPTION;
        let axis = frame.center().y + CAPTION / 2.0;
        Self::scaled(frame, along, 2.0 * half_mm, room_h, axis, cuts)
    }

    /// The support standing on the plate, which runs along the frame's foot: `along` fits
    /// the height, with its end on the plate, and `across` the width, centred.
    fn standing(frame: Rect, along: (f32, f32), across: (f32, f32)) -> Self {
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

    fn scaled(
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
    fn centred(mut self, from_mm: f32, to_mm: f32) -> Self {
        let width = self.x(to_mm) + WALL_W - self.x(from_mm);
        let spare = (self.frame.width() - 2.0 * INSET - width).max(0.0);
        self.origin.x += self.frame.left() + INSET + spare / 2.0 - self.x(from_mm);
        self
    }

    /// The same view moved along so that `x_mm` stands `gap` points in from the right.
    fn anchored_right(mut self, x_mm: f32, gap: f32) -> Self {
        self.origin.x += self.frame.right() - INSET - gap - self.x(x_mm);
        self
    }

    /// Points along the support from its origin to `x_mm`, less what the cuts take out.
    fn along(&self, x_mm: f32) -> f32 {
        let mut pt = x_mm * self.scale;
        for &(from, to) in &self.cuts {
            let inside = (x_mm.min(to) - from).max(0.0);
            pt -= inside * self.scale - BREAK_W * inside / (to - from);
        }
        pt
    }

    fn x(&self, x_mm: f32) -> f32 {
        self.origin.x + self.along(x_mm)
    }

    /// Which way on screen the support runs, and which way across it.
    fn along_dir(&self) -> Vec2 {
        if self.standing { Vec2::Y } else { Vec2::X }
    }

    fn across_dir(&self) -> Vec2 {
        if self.standing { Vec2::X } else { Vec2::Y }
    }

    /// The scale a lying view of `along_mm` lengthwise and `across_mm` across would get
    /// in `frame`, and the same standing.
    fn scales(frame: Rect, along_mm: f32, across_mm: f32) -> (f32, f32) {
        let lying = Self::fit(frame, (0.0, along_mm), across_mm / 2.0, Vec::new()).scale;
        let standing = Self::standing(frame, (0.0, along_mm), (0.0, across_mm)).scale;
        (lying, standing)
    }

    fn y(&self, y_mm: f32) -> f32 {
        self.origin.y + y_mm * self.scale
    }

    fn at(&self, x_mm: f32, y_mm: f32) -> Pos2 {
        if self.standing {
            pos2(
                self.origin.x + y_mm * self.scale,
                self.origin.y + self.along(x_mm),
            )
        } else {
            pos2(self.x(x_mm), self.y(y_mm))
        }
    }

    fn len(&self, mm: f32) -> f32 {
        mm * self.scale
    }

    /// The `row`th row of figures above the axis, clear of `reach` points either side of
    /// it; a negative row is below.
    fn lane(&self, reach: f32, row: i32) -> f32 {
        let off = reach + LANE + LANE_STEP * (row.abs() - 1) as f32;
        if row > 0 {
            self.origin.y - off
        } else {
            self.origin.y + off
        }
    }
}

/// One ring of a support swept about its axis: how far along it stands and half its
/// width, millimetres, and the part the stretch from it to the next belongs to.
#[derive(Clone, Copy)]
struct Station {
    x: f32,
    half: f32,
    part: Part,
}

fn station(x: f32, half: f32, part: Part) -> Station {
    Station { x, half, part }
}

/// Half the width of `stations` at `x_mm`, straight between them.
fn half_at(stations: &[Station], x_mm: f32) -> f32 {
    for pair in stations.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if x_mm >= a.x && x_mm <= b.x {
            let along = if b.x > a.x {
                (x_mm - a.x) / (b.x - a.x)
            } else {
                1.0
            };
            return a.half + (b.half - a.half) * along;
        }
    }
    stations.last().map_or(0.0, |station| station.half)
}

/// A support standing on the plate as the mesher builds it, and where its parts meet,
/// millimetres along.
struct Column {
    stations: Vec<Station>,
    neck: f32,
    flare: f32,
    foot: f32,
    plate: f32,
    /// Half the trunk's width at its base, the flare's where it lands and the pad's.
    base: f32,
    lower: f32,
    pad: f32,
}

impl Column {
    /// One carrying `tips` whose trunk stands `trunk` millimetres tall.
    fn of(profile: &SupportProfile, tips: usize, trunk: f32) -> Self {
        let mut stations = tip(profile);
        let neck = stations.last().map_or(0.0, |station| station.x);
        let base = profile.trunk_base_radius_mm(trunk, tips);
        let foot = neck + trunk;
        let plate = foot + profile.base_height_mm();
        let (lower, feet) = foot_stations(profile, base, foot, plate);
        let flare = foot - (lower - base).max(0.0);
        stations.push(station(flare, base, Part::Base));
        stations.push(station(foot, lower, Part::Base));
        let pad = feet.iter().map(|station| station.half).fold(0.0, f32::max);
        stations.extend(feet);
        Self {
            stations,
            neck,
            flare,
            foot,
            plate,
            base,
            lower,
            pad,
        }
    }

    fn half_at(&self, x_mm: f32) -> f32 {
        half_at(&self.stations, x_mm)
    }

    /// The stations from `x_mm` on, the first of them there.
    fn from(&self, x_mm: f32) -> Vec<Station> {
        let mut rest = vec![station(x_mm, self.half_at(x_mm), Part::Body)];
        rest.extend(self.stations.iter().filter(|station| station.x > x_mm));
        rest
    }

    /// The stations up to the neck.
    fn tip(&self) -> Vec<Station> {
        let tip = self
            .stations
            .iter()
            .filter(|station| station.x <= self.neck);
        tip.copied().collect()
    }
}

/// How tall the trunk is of a support under a part stood as low as the profile lets it,
/// millimetres.
fn lowest_trunk_mm(profile: &SupportProfile) -> f32 {
    let (top, foot) = (profile.top.length_mm, profile.base_height_mm());
    (profile.lift_mm() - top - foot).max(2.0 * top).max(1.0)
}

/// How tall a trunk carrying `tips` stands before its base reaches the cap, millimetres.
fn trunk_height_mm(profile: &SupportProfile, tips: usize) -> f32 {
    let cap = profile.trunk_base_radius_mm(f32::MAX / 4.0, tips);
    let (mut low, mut high) = (0.0_f32, 4096.0_f32);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if profile.trunk_base_radius_mm(middle, tips) >= cap - 1e-5 {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

/// The tip: a plane pushed its bite into the model, or a ball whose top is that far in,
/// then the cone out to the pillar. See `docs/decisions/0243`.
fn tip(profile: &SupportProfile) -> Vec<Station> {
    let (contact, bite) = (profile.contact_radius_mm(), profile.contact_depth_mm());
    let (length, pillar) = (profile.top.length_mm, profile.pillar_radius_mm());
    match profile.tip.shape {
        ContactShape::Plane => {
            let bite = bite.min(length);
            vec![
                station(-bite, contact, Part::Tip),
                station(length - bite, pillar, Part::Body),
            ]
        }
        ContactShape::Sphere => {
            let mut stations = vec![station(-bite, 0.0, Part::Tip)];
            for step in 1..BALL_STEPS {
                let angle = std::f32::consts::PI * step as f32 / BALL_STEPS as f32;
                let x = contact.mul_add(1.0 - angle.cos(), -bite);
                stations.push(station(x, contact * angle.sin(), Part::Tip));
            }
            let last = stations.last().map_or(0.0, |station| station.x);
            stations.push(station(length.max(last), pillar, Part::Body));
            stations
        }
    }
}

/// The flare's half-width where it lands, and the foot from the pad's top to the plate:
/// rings about the axis, or a skate seen along its length.
fn foot_stations(
    profile: &SupportProfile,
    base: f32,
    foot: f32,
    plate: f32,
) -> (f32, Vec<Station>) {
    use core_supports::{FOOT_BEVEL, FOOT_TAPER, SKATE_BEVEL, SKATE_LENGTH, SKATE_WIDTH};
    let shape = profile.bottom.shape;
    let rim = profile.base_radius_mm();
    let bevel = profile.base_height_mm() * FOOT_BEVEL;
    let flare = profile.flare_lower_radius_mm();
    if shape == PlatformShape::Skate {
        let lower = flare.min(rim * SKATE_WIDTH * (1.0 - SKATE_BEVEL)).max(base);
        let long = rim * SKATE_LENGTH;
        let feet = vec![
            station(foot, long, Part::Base),
            station(plate, long * (1.0 - SKATE_BEVEL), Part::Base),
        ];
        return (lower, feet);
    }
    let on_plate = (rim - bevel).max(base);
    let top = if shape.tapers() {
        (rim * FOOT_TAPER).max(base)
    } else {
        rim
    };
    let mut feet = vec![station(foot, top, Part::Base)];
    if shape.tapers() {
        feet.push(station(plate - bevel, rim, Part::Base));
    }
    feet.push(station(plate, on_plate, Part::Base));
    (flare.min(on_plate.min(top)).max(base), feet)
}

/// How one outline is drawn: the accent over a wash when it is the part picked, a plain
/// line otherwise.
#[derive(Clone, Copy, PartialEq)]
enum Pen {
    Picked,
    Plain,
}

impl Pen {
    fn of(part: Part, picked: Part) -> Self {
        if part == picked {
            Self::Picked
        } else {
            Self::Plain
        }
    }

    fn stroke(self) -> Stroke {
        let colors = theme::colors();
        let colour = match self {
            Self::Picked => colors.accent,
            Self::Plain => colors.text_low,
        };
        Stroke::new(LINE_W, colour)
    }

    /// The wash as it lands on the drawing's ground, so pieces that overlap read as one.
    fn fill(self) -> Color32 {
        let colors = theme::colors();
        match self {
            Self::Picked => colors.sunken.blend(colors.accent_wash),
            Self::Plain => Color32::TRANSPARENT,
        }
    }

    /// A convex outline, washed and stroked, or only dashed when it is switched off.
    fn outline(self, points: Vec<Pos2>, off: bool) -> Vec<Shape> {
        if off {
            let mut closed = points.clone();
            closed.extend(points.first().copied());
            return Shape::dashed_line(&closed, self.stroke(), 4.0, 3.0);
        }
        vec![Shape::convex_polygon(points, self.fill(), self.stroke())]
    }
}

/// Draws a regular support with `picked` in the accent and its measurements on it.
pub fn regular(ui: &mut Ui, profile: &SupportProfile, picked: Part) {
    clean_sheet();
    let frame = canvas(ui);
    let painter = ui.painter_at(frame.shrink(1.0));
    match picked {
        Part::Tip => tip_view(&painter, frame, profile),
        Part::Body => body_view(&painter, frame, profile),
        Part::Base => base_view(&painter, frame, profile),
        Part::Raft => raft_view(&painter, frame, profile),
        Part::Brace => brace_view(&painter, frame, profile),
    }
}

/// The tip close up: the contact, how far it is pushed in, and the cone's length.
fn tip_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let column = Column::of(profile, 1, lowest_trunk_mm(profile));
    let (bite, length) = (profile.contact_depth_mm(), profile.top.length_mm);
    let ball = match profile.tip.shape {
        ContactShape::Plane => 0.0,
        ContactShape::Sphere => 2.0 * profile.contact_radius_mm(),
    };
    let span = bite + length.max(ball);
    let end = length.max(ball) + span * 0.4;
    let widest = profile.contact_radius_mm().max(profile.pillar_radius_mm());
    // Twice over: the room the contact's figure takes inside the model depends on the scale.
    let fitted = View::fit(frame, (-bite, end), widest, Vec::new());
    let room = (TIP_LABEL_W / fitted.scale).max(span * 0.6);
    let view = View::fit(frame, (-bite - room, end), widest, Vec::new());
    model_wall(painter, &view);
    revolved(painter, &view, &column.stations, 0.0, Part::Tip);
    tip_figures(painter, &view, profile, ball);
}

/// The contact's width, how far the tip is pushed in and how long its cone is.
fn tip_figures(painter: &Painter, view: &View, profile: &SupportProfile, ball: f32) {
    let (bite, length) = (profile.contact_depth_mm(), profile.top.length_mm);
    let contact = view.len(profile.contact_radius_mm());
    let reach = contact.max(view.len(profile.pillar_radius_mm()));
    // A plane tip goes no further in than its own cone is long, as the mesher builds it.
    let sunk = match profile.tip.shape {
        ContactShape::Plane => bite.min(length),
        ContactShape::Sphere => bite,
    };
    let (start, end) = match profile.tip.shape {
        ContactShape::Plane => (-sunk, length - sunk),
        ContactShape::Sphere => (0.0, length.max(ball - bite)),
    };
    // Inside the model, clear of the tip's own edge, with lines out to where it is widest.
    let (rim, front) = match profile.tip.shape {
        ContactShape::Plane => (view.x(start), view.x(start)),
        ContactShape::Sphere => (view.x(contact_centre(profile)), view.x(-bite)),
    };
    let x = front - 12.0;
    let y = view.origin.y;
    let extension = Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.5));
    for side in [-contact, contact] {
        painter.line_segment([pos2(rim, y + side), pos2(x - 3.0, y + side)], extension);
    }
    let above = (pos2(x, view.lane(reach, 1)), Align2::CENTER_CENTER);
    let label = diameter(profile.tip.contact_diameter_mm);
    across(painter, pos2(x, y), contact, &label, None, above);
    let length_y = view.lane(reach, 2);
    let from = pos2(view.x(start), length_y);
    dimension(painter, from, pos2(view.x(end), length_y), &mm(length));
    let bite_y = view.lane(reach, -1);
    dimension(
        painter,
        pos2(view.x(-sunk), bite_y),
        pos2(view.x(0.0), bite_y),
        "",
    );
    let beside = pos2(view.x(0.0) + ARROW * 3.0, bite_y);
    text(painter, beside, Align2::LEFT_CENTER, &mm(bite));
}

/// Where a sphere tip is widest, millimetres along: its centre.
fn contact_centre(profile: &SupportProfile) -> f32 {
    profile.contact_radius_mm() - profile.contact_depth_mm()
}

/// Two tips the merge distance apart, their branches meeting at the branch angle, the
/// trunk they stand on widening to the cap, and the air it keeps.
fn body_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let branching = &profile.branching;
    let tips = if branching.enabled { 2 } else { 1 };
    let spread = if branching.enabled {
        branching.max_merge_distance_mm / 2.0
    } else {
        0.0
    };
    let pillar = profile.pillar_radius_mm();
    let widest_mm = profile.branching.max_trunk_diameter_mm.max(2.0 * pillar);
    let merging = spread / branching.reach_per_drop();
    let bite = profile.contact_depth_mm();
    let widest = spread + profile.contact_radius_mm().max(pillar);
    let trunk = trunk_height_mm(profile, tips).max(merging + 4.0 * widest_mm);
    let trunk = trunk + spare_length_mm(frame, profile, widest, trunk + merging);
    let column = Column::of(profile, tips, trunk);
    let merge = column.neck + merging;
    let start = merge + 4.0 * pillar.max(column.base);
    let end = column.flare - 4.0 * column.base;
    let cuts = if end > start + pillar {
        vec![(start, end)]
    } else {
        Vec::new()
    };
    let along = (-bite - (merge + bite) * 0.1, column.plate);
    let view = View::fit(frame, along, widest, cuts).centred(along.0, column.plate);
    plate_wall(painter, &view, column.plate);
    if branching.enabled {
        branched(painter, &view, profile, &column, spread);
        branch_figures(painter, &view, profile, &column, spread);
    } else {
        revolved(painter, &view, &column.stations, 0.0, Part::Body);
        let over = view.lane(view.len(pillar) + view.len(profile.clearance_mm), 1);
        let at = pos2(view.x(merge) + 40.0, over);
        text(painter, at, Align2::LEFT_CENTER, "no branching");
    }
    breaks(painter, &view, &column.stations);
    body_figures(painter, &view, profile, &column, merge);
}

/// How much longer than `length_mm` a support `half_mm` either side of its axis may be
/// drawn before it runs out of the frame, millimetres: what the trunk takes up, so a
/// support short for its width fills the frame rather than sitting small in it.
fn spare_length_mm(frame: Rect, profile: &SupportProfile, half_mm: f32, length_mm: f32) -> f32 {
    let scale = View::fit(frame, (0.0, 1e-3), half_mm, Vec::new()).scale;
    let fits = (frame.width() - 2.0 * INSET - WALL_W) / scale;
    let ends = profile.top.length_mm + profile.base_height_mm() + profile.contact_depth_mm();
    (fits - length_mm - ends * 1.2).max(0.0)
}

/// The two tips, their struts and the trunk they merge into, washed and outlined as one
/// solid: each strut stops where it meets the trunk's edge or the other strut.
fn branched(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    spread: f32,
) {
    let pillar = view.len(profile.pillar_radius_mm());
    let joint = view.at(
        column.neck + spread / profile.branching.reach_per_drop(),
        0.0,
    );
    let pen = Pen::Picked;
    let mut struts = Vec::new();
    for side in [-1.0, 1.0] {
        revolved(painter, view, &column.tip(), side * spread, Part::Body);
        struts.push(Strut::of(
            view.at(column.neck, side * spread),
            joint,
            pillar,
            side,
        ));
    }
    let start = struts[0].apex.x;
    let trunk_from = column.neck + (start - view.x(column.neck)) / view.scale;
    revolved(painter, view, &column.from(trunk_from), 0.0, Part::Body);
    for strut in &struts {
        painter.add(Shape::convex_polygon(
            strut.fill(),
            pen.fill(),
            Stroke::NONE,
        ));
        painter.add(Shape::circle_filled(strut.neck, pillar, pen.fill()));
    }
    for strut in &struts {
        strut.stroke(painter, pen.stroke(), pillar);
    }
}

/// One branch from its tip's neck in to the merge, in points.
struct Strut {
    neck: Pos2,
    /// The edges away from the axis and towards it, each as where it starts at the neck
    /// and where it stops: on the trunk's edge, and on the axis where it meets the other.
    outer: [Pos2; 2],
    inner: [Pos2; 2],
    apex: Pos2,
}

impl Strut {
    fn of(neck: Pos2, joint: Pos2, half: f32, side: f32) -> Self {
        let along = (joint - neck).normalized();
        let normal = vec2(along.y, -along.x);
        let (out, inn) = if normal.y * side > 0.0 {
            (normal, -normal)
        } else {
            (-normal, normal)
        };
        let reach = |from: Pos2, y: f32| from + along * ((y - from.y) / along.y);
        let outer_start = neck + out * half;
        let inner_start = neck + inn * half;
        let apex = reach(inner_start, joint.y);
        Self {
            neck,
            outer: [outer_start, reach(outer_start, joint.y + side * half)],
            inner: [inner_start, apex],
            apex,
        }
    }

    fn fill(&self) -> Vec<Pos2> {
        vec![self.outer[0], self.outer[1], self.inner[1], self.inner[0]]
    }

    /// The strut's two edges, and the round of the joint on the outside of its bend.
    fn stroke(&self, painter: &Painter, stroke: Stroke, half: f32) {
        painter.line_segment(self.outer, stroke);
        let corner = pos2(
            self.neck.x,
            self.neck.y + (self.outer[0].y - self.neck.y).signum() * half,
        );
        let tip_side = (corner - self.neck).angle();
        let mut strut_side = (self.outer[0] - self.neck).angle();
        if (strut_side - tip_side).abs() > std::f32::consts::PI {
            strut_side += std::f32::consts::TAU * (tip_side - strut_side).signum();
        }
        let points: Vec<Pos2> = (0..=8)
            .map(|step| tip_side + (strut_side - tip_side) * step as f32 / 8.0)
            .map(|angle| self.neck + Vec2::angled(angle) * half)
            .collect();
        painter.add(Shape::line(points, stroke));
        let tip_edge = self.neck.y + (self.inner[0].y - self.neck.y).signum() * half;
        let along = self.inner[1] - self.inner[0];
        let meet = if along.y.abs() > f32::EPSILON {
            self.inner[0] + along * ((tip_edge - self.inner[0].y) / along.y)
        } else {
            self.inner[0]
        };
        painter.line_segment([meet, self.inner[1]], stroke);
    }
}

/// The merge distance between the two tips and the branch angle at the merge.
fn branch_figures(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    spread: f32,
) {
    let branching = &profile.branching;
    let span_x = view.x(0.0) + 3.0;
    let (top, bottom) = (view.y(-spread), view.y(spread));
    dimension(painter, pos2(span_x, top), pos2(span_x, bottom), "");
    let spacing = format!("≤{:.1}", branching.max_merge_distance_mm);
    let beside = pos2(span_x + 4.0, view.origin.y);
    text(painter, beside, Align2::LEFT_CENTER, &spacing);
    // Off the tip's own axis at the upper neck, outside the V the two branches make.
    let joint = view.at(column.neck + spread / branching.reach_per_drop(), 0.0);
    let neck = view.at(column.neck, -spread);
    let r = ((neck - joint).length() * 0.5).max(14.0);
    let lean = (joint - neck).angle();
    let guide = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(
        &[neck, neck + vec2(r + 6.0, 0.0)],
        guide,
        4.0,
        3.0,
    ));
    arc(painter, neck, r, 0.0, lean);
    let angle = format!("{:.0}°", branching.max_angle_deg);
    let at = neck + Vec2::angled(lean / 2.0) * (r + 12.0);
    text(painter, at, Align2::CENTER_CENTER, &angle);
}

/// The pillar's width, the air the body keeps, and the widest its trunk's base grows.
fn body_figures(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    merge: f32,
) {
    let clear = clearance(painter, view, profile, column, merge);
    let pillar = view.len(profile.pillar_radius_mm());
    let x = view.x(merge) + (pillar * 2.0).max(10.0);
    let row = view.lane(view.len(column.half_at(merge)) + clear, -1);
    let below = (pos2(x - 4.0, row), Align2::LEFT_CENTER);
    let width = diameter(profile.middle.diameter_mm);
    across(painter, pos2(x, view.origin.y), pillar, &width, None, below);
    let x = view.x(column.flare) - 6.0;
    let base = view.len(column.base);
    let below = (pos2(x, view.lane(base + clear, -2)), Align2::RIGHT_CENTER);
    let widest = format!("Ø≤{:.2}", profile.branching.max_trunk_diameter_mm);
    let inside = Some(Align2::RIGHT_CENTER);
    across(
        painter,
        pos2(x, view.origin.y),
        base,
        &widest,
        inside,
        below,
    );
}

/// The air the trunk keeps, dashed along its underside, its width written in the gap.
/// Returns the gap in points.
fn clearance(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    merge: f32,
) -> f32 {
    let clear = profile.clearance_mm;
    let edge = |x: f32| view.at(x, column.half_at(x) + clear);
    let mut line: Vec<Pos2> = split_at_cuts(&column.from(merge), &view.cuts)
        .iter()
        .filter(|station| station.x <= column.flare)
        .map(|station| edge(station.x))
        .collect();
    line.dedup();
    let air = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(&line, air, 4.0, 3.0));
    // Beside the pillar's own width, which is written under the dashed line.
    let mark = view.x(merge) + (view.len(profile.pillar_radius_mm()) * 2.0).max(10.0) + 30.0;
    let below = view.origin.y + view.len(column.half_at(merge));
    let (from, to) = (pos2(mark, below), pos2(mark, below + view.len(clear)));
    dimension(painter, from, to, "");
    let beside = pos2(mark + 4.0, from.lerp(to, 0.5).y);
    text(painter, beside, Align2::LEFT_CENTER, &mm(clear));
    view.len(clear)
}

/// The foot standing on the plate: the flare's width where it lands, the pad's width and
/// its thickness.
fn base_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let column = Column::of(profile, 1, lowest_trunk_mm(profile));
    let height = column.plate - column.flare;
    let along = (column.flare - height.max(column.pad * 0.3), column.plate);
    let view = View::standing(frame, along, (-column.pad, column.pad));
    plate_floor(painter, &view, column.plate);
    revolved(painter, &view, &column.stations, 0.0, Part::Base);
    facets(painter, &view, profile, &column);

    let (foot, plate) = (column.foot, column.plate);
    // Over the flare's top, on lines up from where it lands, so it crosses nothing.
    let (left, right) = (view.at(foot, -column.lower), view.at(foot, column.lower));
    let level = view.at(column.flare, 0.0).y - LANE;
    let extension = Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.5));
    for corner in [left, right] {
        painter.line_segment([corner, pos2(corner.x, level - 3.0)], extension);
    }
    dimension(painter, pos2(left.x, level), pos2(right.x, level), "");
    let low = diameter(profile.bottom.lower_diameter_mm);
    text(
        painter,
        pos2(left.x - 6.0, level),
        Align2::RIGHT_CENTER,
        &low,
    );
    let middle = (view.at(foot, 0.0).y + view.at(plate, 0.0).y) / 2.0;
    let (edge, far) = (view.at(foot, -column.pad).x, view.at(foot, column.pad).x);
    let width = diameter(profile.bottom.platform_diameter_mm);
    dimension(painter, pos2(edge, middle), pos2(far, middle), &width);
    let side = far + 8.0;
    let (top, bottom) = (view.at(foot, 0.0).y, view.at(plate, 0.0).y);
    dimension(painter, pos2(side, top), pos2(side, bottom), "");
    let thickness = mm(profile.bottom.platform_thickness_mm);
    text(
        painter,
        pos2(side + 4.0, middle),
        Align2::LEFT_CENTER,
        &thickness,
    );
}

/// The vertical edges of a prism or cube foot, where its sides meet, seen side on.
fn facets(painter: &Painter, view: &View, profile: &SupportProfile, column: &Column) {
    let shape = profile.bottom.shape;
    if !matches!(shape, PlatformShape::Prism | PlatformShape::Cube) {
        return;
    }
    let sides = shape.sides(profile.facets);
    let stroke = Pen::Plain.stroke();
    for corner in 0..sides {
        let angle = std::f32::consts::TAU * (corner as f32 + 0.5) / sides as f32;
        if angle.cos() < 0.0 {
            continue;
        }
        let across = angle.sin();
        let top = view.at(column.foot, across * column.half_at(column.foot));
        let bottom = view.at(column.plate, across * column.half_at(column.plate));
        painter.line_segment([top, bottom], stroke);
    }
}

/// The rim of the raft standing on the plate, cut through the middle of one foot, from
/// inside the pad out to its edge: its thickness, its wall's lean, how far it reaches
/// past the pad, and its style.
fn raft_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let column = Column::of(profile, 1, lowest_trunk_mm(profile));
    let section = RaftSection::of(profile);
    let rim = section.reach - section.pad;
    let along = (column.plate - section.tall * 2.5, column.plate);
    let across = ((section.pad - rim).max(0.0), section.reach + rim * 0.1);
    let view = View::standing(frame, along, across);
    plate_floor(painter, &view, column.plate);
    section.draw(painter, &view, &column, !profile.raft.enabled);
    revolved(painter, &view, &column.stations, 0.0, Part::Raft);
    raft_figures(painter, &view, profile, &column, &section);
}

/// A raft cut through the middle of one foot, millimetres: half its width where the
/// supports stand on it and where it meets the plate, how thick and how tall it is.
struct RaftSection {
    top: f32,
    bottom: f32,
    thick: f32,
    tall: f32,
    reach: f32,
    pad: f32,
    style: RaftStyle,
}

impl RaftSection {
    fn of(profile: &SupportProfile) -> Self {
        use core_supports::{GRID_BAR_MM, LIP_WIDTH_MM};
        let raft = &profile.raft;
        let (thick, overhang) = (raft.thickness_mm, raft.overhang_mm());
        let pad = profile.base_radius_mm();
        let margin = raft.margin_mm.max(0.0);
        let (top, tall) = match raft.style {
            RaftStyle::Skates => (
                pad + margin.max(2.0 * (overhang + LIP_WIDTH_MM)),
                2.0 * thick,
            ),
            RaftStyle::Grid => (pad + margin.max(2.0 * (overhang + GRID_BAR_MM)), thick),
            RaftStyle::Connected => (pad + margin, thick),
        };
        Self {
            top,
            bottom: top - overhang,
            thick,
            tall,
            reach: top + overhang,
            pad,
            style: raft.style,
        }
    }

    fn draw(&self, painter: &Painter, view: &View, column: &Column, off: bool) {
        let pen = Pen::Picked;
        if self.style == RaftStyle::Skates {
            let outline = self.saucer(view, column.plate);
            if off {
                let mut closed = outline.clone();
                closed.extend(outline.first().copied());
                painter.extend(Shape::dashed_line(&closed, pen.stroke(), 4.0, 3.0));
                return;
            }
            for piece in self.saucer_pieces(view, column.plate) {
                painter.add(Shape::convex_polygon(piece, pen.fill(), Stroke::NONE));
            }
            painter.add(Shape::closed_line(outline, pen.stroke()));
            return;
        }
        for piece in self.pieces(view, column.plate) {
            painter.extend(pen.outline(piece, off));
        }
    }

    /// A slab between `from` and `to` across, its walls sloped where it ends at the rim.
    fn slab(&self, view: &View, plate: f32, from: f32, to: f32) -> Vec<Pos2> {
        let face = plate - self.thick;
        let lean = |y: f32| {
            if y.abs() >= self.top - 1e-4 {
                y.signum() * self.bottom
            } else {
                y
            }
        };
        vec![
            view.at(face, from),
            view.at(face, to),
            view.at(plate, lean(to)),
            view.at(plate, lean(from)),
        ]
    }

    /// The grid's rim, bars and the pad under the foot, or the connected raft's pad.
    fn pieces(&self, view: &View, plate: f32) -> Vec<Vec<Pos2>> {
        use core_supports::{GRID_BAR_MM, GRID_HOLE_MM};
        // The strip to the next foot runs across the cut, a foot wide, behind the pad.
        if self.style == RaftStyle::Connected {
            return vec![self.slab(view, plate, -self.top, self.top)];
        }
        let rim = self.top - (self.top - self.bottom) - GRID_BAR_MM;
        let mut pieces = vec![
            self.slab(view, plate, -self.top, -rim),
            self.slab(view, plate, rim, self.top),
            self.slab(view, plate, -self.pad, self.pad),
        ];
        let mut at = self.pad + GRID_HOLE_MM;
        while at + GRID_BAR_MM < rim - GRID_HOLE_MM / 2.0 {
            pieces.push(self.slab(view, plate, at, at + GRID_BAR_MM));
            pieces.push(self.slab(view, plate, -at - GRID_BAR_MM, -at));
            at += GRID_BAR_MM + GRID_HOLE_MM;
        }
        pieces
    }

    /// The skate raft's section all the way round: the floor, the walls leaning out up
    /// into the lip, and the lip's inner face.
    fn saucer(&self, view: &View, plate: f32) -> Vec<Pos2> {
        use core_supports::LIP_WIDTH_MM;
        let (face, lip) = (plate - self.thick, plate - self.tall);
        let rim = self.top + (self.top - self.bottom);
        let side = |sign: f32| {
            [
                view.at(face, sign * (self.top - LIP_WIDTH_MM)),
                view.at(lip, sign * (rim - LIP_WIDTH_MM)),
                view.at(lip, sign * rim),
                view.at(plate, sign * self.bottom),
            ]
        };
        let mut outline: Vec<Pos2> = side(-1.0).to_vec();
        outline.extend(side(1.0).iter().rev());
        outline
    }

    /// The saucer cut into pieces each convex, to wash: the floor and each side's lip.
    fn saucer_pieces(&self, view: &View, plate: f32) -> Vec<Vec<Pos2>> {
        use core_supports::LIP_WIDTH_MM;
        let (face, lip) = (plate - self.thick, plate - self.tall);
        let rim = self.top + (self.top - self.bottom);
        let mut pieces = vec![self.slab(view, plate, -self.top, self.top)];
        for sign in [-1.0, 1.0] {
            pieces.push(vec![
                view.at(face, sign * (self.top - LIP_WIDTH_MM)),
                view.at(lip, sign * (rim - LIP_WIDTH_MM)),
                view.at(lip, sign * rim),
                view.at(face, sign * self.top),
            ]);
        }
        pieces
    }
}

/// The slab's thickness, how far it reaches past the pad, and the lean of its wall.
fn raft_figures(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    section: &RaftSection,
) {
    let raft = &profile.raft;
    let face = column.plate - section.thick;
    let above = view.at(column.plate - section.tall, 0.0).y - LANE;
    if !raft.enabled {
        let at = pos2(view.at(face, section.top).x, above);
        text(painter, at, Align2::CENTER_BOTTOM, "no raft");
        return;
    }
    let (pad, edge) = (view.at(face, section.pad).x, view.at(face, section.top).x);
    dimension(painter, pos2(pad, above), pos2(edge, above), "");
    let margin = format!("+{}", mm(raft.margin_mm));
    text(
        painter,
        pos2(pad - 4.0, above),
        Align2::RIGHT_CENTER,
        &margin,
    );
    let side = view.at(face, section.reach).x + 8.0;
    let (top, bottom) = (view.at(face, 0.0).y, view.at(column.plate, 0.0).y);
    dimension(painter, pos2(side, top), pos2(side, bottom), "");
    let middle = (top + bottom) / 2.0;
    text(
        painter,
        pos2(side + 4.0, middle),
        Align2::LEFT_CENTER,
        &mm(raft.thickness_mm),
    );
    // Off the plate's normal, at the corner where the wall leaves the supports' face.
    let corner = view.at(face, section.top);
    let foot = view.at(column.plate, section.bottom);
    let r = ((foot - corner).length() * 0.8).max(14.0);
    let down = std::f32::consts::FRAC_PI_2;
    let normal = [corner, corner + vec2(0.0, r + 4.0)];
    let guide = Stroke::new(1.0, theme::colors().text_low);
    painter.extend(Shape::dashed_line(&normal, guide, 3.0, 2.0));
    arc(painter, corner, r, (foot - corner).angle(), down);
    let slope = format!("{:.0}°", raft.slope_deg);
    let mid = ((foot - corner).angle() + down) / 2.0;
    text(
        painter,
        corner + Vec2::angled(mid) * (r + 12.0),
        Align2::CENTER_CENTER,
        &slope,
    );
}

/// Two trunks the spacing apart standing on the plate, the crossed braces tying them, and
/// where they go.
fn brace_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let bracing = &profile.bracing;
    let gap = bracing.max_spacing_mm / 2.0;
    let rise = bracing.rise_mm;
    let levels = BRACE_LEVELS as f32;
    let trunk = bracing.start_height_mm + rise * (levels + 1.0);
    let column = Column::of(profile, 1, trunk);
    let first = column.plate - bracing.start_height_mm;
    let top = first - rise * levels;
    let half = gap + column.base.max(profile.pillar_radius_mm());
    let view = brace_frame(frame, (top, column.plate), half);
    if view.standing {
        plate_floor(painter, &view, column.plate);
    } else {
        plate_wall(painter, &view, column.plate);
    }
    for side in [-1.0, 1.0] {
        revolved(painter, &view, &column.stations, side * gap, Part::Brace);
    }
    for level in 0..BRACE_LEVELS {
        let near = first - rise * level as f32;
        let far = near - rise;
        let thick = brace_half(profile, &column, near, far);
        for (from, to) in [(-gap, gap), (gap, -gap)] {
            let off = !bracing.enabled;
            strut(painter, &view, (near, from), (far, to), thick, off);
        }
    }
    brace_figures(painter, &view, profile, &column, (first, top, gap));
}

/// The braces lying or standing, whichever shows `along` and `half` either side of the
/// axis the larger, with room before `along` for the figures written there.
fn brace_frame(frame: Rect, along: (f32, f32), half: f32) -> View {
    let (lying, standing) = View::scales(frame, along.1 - along.0, 2.0 * half);
    if lying >= standing {
        let start = along.0 - (BRACE_LABEL_W + WALL_W) / lying;
        View::fit(frame, (start, along.1), half, Vec::new()).anchored_right(along.1, WALL_W)
    } else {
        let start = along.0 - (2.0 * FIGURE_H + LANE) / standing;
        View::standing(frame, (start, along.1), (-half, half))
    }
}

/// Half a brace's width between trunks `near` and `far` along, millimetres: never wider
/// than either trunk where it meets it (ADR 0242).
fn brace_half(profile: &SupportProfile, column: &Column, near: f32, far: f32) -> f32 {
    let bracing = profile.bracing.radius_mm();
    bracing.min(column.half_at(near)).min(column.half_at(far))
}

/// One brace from `from` to `to`, millimetres, `thick` either side of its line.
fn strut(painter: &Painter, view: &View, from: (f32, f32), to: (f32, f32), thick: f32, off: bool) {
    let (a, b) = (view.at(from.0, from.1), view.at(to.0, to.1));
    let side = (b - a).normalized().rot90() * view.len(thick);
    let outline = vec![a - side, b - side, b + side, a + side];
    painter.extend(Pen::Picked.outline(outline, off));
}

/// Where the first brace goes, how far apart the levels are, how far apart the trunks
/// may be and how wide a brace is, each written on the side away from the braces.
fn brace_figures(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    (first, top, gap): (f32, f32, f32),
) {
    let bracing = &profile.bracing;
    let (along, across) = (view.along_dir(), view.across_dir());
    let clear = across * (view.len(column.half_at(first)) + LANE);
    let every = format!("every {}", mm(bracing.rise_mm));
    let (from, to) = (view.at(first - bracing.rise_mm, -gap), view.at(first, -gap));
    dimension(painter, from - clear, to - clear, "");
    beside(
        painter,
        (from - clear).lerp(to - clear, 0.5),
        -across,
        &every,
    );
    let start = format!("first {}", mm(bracing.start_height_mm));
    let (from, to) = (view.at(first, gap), view.at(column.plate, gap));
    dimension(painter, from + clear, to + clear, "");
    beside(
        painter,
        (from + clear).lerp(to + clear, 0.5),
        across,
        &start,
    );
    let back = along * 8.0;
    let (from, to) = (view.at(top, -gap) - back, view.at(top, gap) - back);
    dimension(painter, from, to, "");
    let middle = from.lerp(to, 0.5);
    let spacing = format!("≤{:.1}", bracing.max_spacing_mm);
    beside(painter, middle, -along, &spacing);
    beside(painter, middle, -along, &diameter(bracing.diameter_mm));
    if !bracing.enabled {
        beside(painter, view.at(top, -gap) - clear, -across, "no bracing");
    }
}

/// A figure written next to `at`, on its `out` side and clear of it.
fn beside(painter: &Painter, at: Pos2, out: Vec2, label: &str) {
    let width = label_width(painter, label);
    let reach = out.x.abs() * (width / 2.0 + 4.0) + out.y.abs() * (FIGURE_H / 2.0 + 2.0);
    text(painter, at + out * reach, Align2::CENTER_CENTER, label);
}

/// Draws the thin strut between two parts of the model, both ends sunk into a wall, the
/// stretch between them cut out.
pub fn small_pillar(ui: &mut Ui, profile: &SupportProfile) {
    clean_sheet();
    let frame = canvas(ui);
    let painter = ui.painter_at(frame.shrink(1.0));
    let pillar = &profile.small_pillar;
    let half = pillar.diameter_mm / 2.0;
    let (upper, lower) = (pillar.upper_depth_mm, pillar.lower_depth_mm);
    let shown = (pillar.diameter_mm * 4.0).max(upper.max(lower) * 3.0);
    let length = shown * 4.0;
    let along = (-upper - shown * 0.6, length + lower + shown * 0.6);
    let view = View::fit(frame, along, half, vec![(shown, length - shown)]);
    model_wall(&painter, &view);
    far_wall(&painter, &view, length);
    let stations = [
        station(-upper, half, Part::Body),
        station(length + lower, half, Part::Body),
    ];
    let pen = |_| Pen::Picked;
    draw_revolved(&painter, &view, &stations, 0.0, &pen, !pillar.enabled);
    breaks(&painter, &view, &stations);

    let y = view.origin.y;
    let thick = view.len(half);
    let middle = view.x(shown / 2.0);
    let above = (pos2(middle, view.lane(thick, 1)), Align2::CENTER_CENTER);
    let width = diameter(pillar.diameter_mm);
    across(&painter, pos2(middle, y), thick, &width, None, above);
    let under = view.lane(thick, -1);
    let (left, right) = (view.x(0.0), view.x(length));
    dimension(&painter, pos2(view.x(-upper), under), pos2(left, under), "");
    let far = pos2(view.x(length + lower), under);
    dimension(&painter, pos2(right, under), far, "");
    let upper_at = pos2(left + ARROW * 3.0, under);
    text(&painter, upper_at, Align2::LEFT_CENTER, &mm(upper));
    let lower_at = pos2(right - ARROW * 3.0, under);
    text(&painter, lower_at, Align2::RIGHT_CENTER, &mm(lower));
    let over = view.lane(thick, 2);
    let longest = format!("max {}", mm(pillar.max_length_mm));
    dimension(&painter, pos2(left, over), pos2(right, over), &longest);
}

/// The body swept about an axis `offset_mm` off the view's, each stretch in its part's
/// pen, with `picked` in the accent.
fn revolved(painter: &Painter, view: &View, stations: &[Station], offset_mm: f32, picked: Part) {
    let pen = |part: Part| Pen::of(part, picked);
    draw_revolved(painter, view, stations, offset_mm, &pen, false);
}

/// `stations` as bands washed one by one, their edges stroked, and a line across wherever
/// one part gives way to another and at both ends; only the edges, dashed, when `off`.
fn draw_revolved(
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
fn split_at_cuts(stations: &[Station], cuts: &[(f32, f32)]) -> Vec<Station> {
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
fn breaks(painter: &Painter, view: &View, stations: &[Station]) {
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

/// The model's face where the support starts, hatched out to the frame's left.
fn model_wall(painter: &Painter, view: &View) {
    let face = view.x(0.0);
    let left = view.frame.left() + INSET;
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(left.min(face - WALL_W)..=face, top..=bottom),
    );
    let at = pos2(left, view.frame.top() + INSET);
    caption(painter, at, Align2::LEFT_TOP, "model");
}

/// A face of the model at `x_mm` hatched out to the frame's right, with its caption.
fn far_wall(painter: &Painter, view: &View, x_mm: f32) {
    let face = view.x(x_mm);
    let right = view.frame.right() - INSET;
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(face..=right.max(face + WALL_W), top..=bottom),
    );
    let at = pos2(right, view.frame.top() + INSET);
    caption(painter, at, Align2::RIGHT_TOP, "model");
}

/// The plate at `x_mm`, a wall's width thick, with its caption over it.
fn plate_wall(painter: &Painter, view: &View, x_mm: f32) {
    let face = view.x(x_mm);
    let (top, bottom) = wall_span(view.frame);
    hatch(
        painter,
        Rect::from_x_y_ranges(face..=face + WALL_W, top..=bottom),
    );
    let at = pos2(face + WALL_W, view.frame.top() + INSET);
    caption(painter, at, Align2::RIGHT_TOP, "plate");
}

/// The plate under a standing support, hatched down to the frame's foot.
fn plate_floor(painter: &Painter, view: &View, x_mm: f32) {
    let top = view.at(x_mm, 0.0).y;
    let (left, right) = (view.frame.left() + INSET, view.frame.right() - INSET);
    let bottom = (view.frame.bottom() - INSET).max(top + WALL_W);
    hatch(painter, Rect::from_x_y_ranges(left..=right, top..=bottom));
    // Written inside the floor, on a patch, where nothing above it can reach.
    let at = pos2(right - 4.0, (top + bottom) / 2.0);
    let (font, colour) = (theme::figures(10.0), theme::colors().text_low);
    let galley = painter.layout_no_wrap("plate".to_owned(), font, colour);
    let rect = Align2::RIGHT_CENTER.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand2(vec2(3.0, 0.0)), 2.0, theme::colors().sunken);
    painter.galley(rect.min, galley, colour);
}

/// The span the walls stand over, top to bottom, leaving the captions their row.
fn wall_span(frame: Rect) -> (f32, f32) {
    (frame.top() + INSET + CAPTION, frame.bottom() - INSET)
}

fn caption(painter: &Painter, at: Pos2, align: Align2, label: &str) {
    let (font, colour) = (theme::figures(10.0), theme::colors().text_low);
    painter.text(at, align, label, font, colour);
}

/// A wall the drawing stands against, as a draughtsman hatches a section.
fn hatch(painter: &Painter, rect: Rect) {
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

/// A diameter: the arrow across `half` points either side of `centre`. Its figure stands
/// beside the arrow, aligned by `inside`, when the width fits it; otherwise at `outside`,
/// joined by a leader.
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
    let wanted = align.anchor_size(at, galley.size());
    let rect = free_spot(wanted, painter.clip_rect().shrink(2.0));
    painter.rect_filled(rect.expand2(vec2(2.0, 0.0)), 2.0, colors.sunken);
    painter.galley(rect.min, galley, colors.accent_soft);
}

thread_local! {
    /// The figures written on the drawing in hand, so a later one keeps clear of them.
    static WRITTEN: RefCell<Vec<Rect>> = const { RefCell::new(Vec::new()) };
}

/// Starts a drawing with nothing written on it yet.
fn clean_sheet() {
    WRITTEN.with(|written| written.borrow_mut().clear());
}

/// Where a figure wanted at `wanted` goes: moved inside `room`, and then a row at a time
/// up or down, nearest first, until it covers no figure already written. Whatever the
/// numbers, no figure is cut off by the frame or written over another.
fn free_spot(wanted: Rect, room: Rect) -> Rect {
    let inside = |rect: Rect| {
        let dx = (room.left() - rect.left()).max(0.0) + (room.right() - rect.right()).min(0.0);
        let dy = (room.top() - rect.top()).max(0.0) + (room.bottom() - rect.bottom()).min(0.0);
        rect.translate(vec2(dx, dy))
    };
    let row = wanted.height() + 2.0;
    WRITTEN.with(|written| {
        let mut written = written.borrow_mut();
        let clear = |rect: &Rect| {
            let padded = rect.expand2(vec2(3.0, 1.0));
            !written.iter().any(|other| other.intersects(padded))
        };
        let spot = (0..12)
            .map(|step| {
                let reach = ((step + 1) / 2) as f32 * row;
                let up = if step % 2 == 1 { -reach } else { reach };
                inside(wanted.translate(vec2(0.0, up)))
            })
            .find(clear)
            .unwrap_or_else(|| inside(wanted));
        written.push(spot);
        spot
    })
}
