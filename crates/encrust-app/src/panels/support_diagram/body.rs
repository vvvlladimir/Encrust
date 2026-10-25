use super::column::{Column, lowest_trunk_mm, trunk_height_mm};
use super::draw::{
    Pen, across, arc, breaks, dimension, plate_floor, plate_wall, revolved, split_at_cuts, text,
};
use super::view::View;
use super::{INSET, LANE, Part, WALL_W, diameter, mm};
use crate::i18n::tr;
use crate::ui::theme;
use egui::{Align2, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use printer_profiles::{PlatformShape, SupportProfile};

/// Two tips the merge distance apart, their branches meeting at the branch angle, the
/// trunk they stand on widening to the cap, and the air it keeps.
pub(super) fn body_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
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
    let bite = profile.tip.contact_depth_mm;
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
        text(
            painter,
            at,
            Align2::LEFT_CENTER,
            &tr!("diagram-no-branching"),
        );
    }
    breaks(painter, &view, &column.stations);
    body_figures(painter, &view, profile, &column, merge);
}

/// How much longer than `length_mm` a support `half_mm` either side of its axis may be drawn
/// before leaving the frame, millimetres, so a short support fills the frame.
pub(super) fn spare_length_mm(
    frame: Rect,
    profile: &SupportProfile,
    half_mm: f32,
    length_mm: f32,
) -> f32 {
    let scale = View::fit(frame, (0.0, 1e-3), half_mm, Vec::new()).scale;
    let fits = (frame.width() - 2.0 * INSET - WALL_W) / scale;
    let ends =
        profile.top.length_mm + profile.bottom.platform_thickness_mm + profile.tip.contact_depth_mm;
    (fits - length_mm - ends * 1.2).max(0.0)
}

/// The two tips, their struts and the trunk they merge into, washed and outlined as one
/// solid: each strut stops where it meets the trunk's edge or the other strut.
pub(super) fn branched(
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
pub(super) struct Strut {
    pub(super) neck: Pos2,
    /// The edges away from the axis and towards it, each as where it starts at the neck
    /// and where it stops: on the trunk's edge, and on the axis where it meets the other.
    pub(super) outer: [Pos2; 2],
    pub(super) inner: [Pos2; 2],
    pub(super) apex: Pos2,
}

impl Strut {
    pub(super) fn of(neck: Pos2, joint: Pos2, half: f32, side: f32) -> Self {
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

    pub(super) fn fill(&self) -> Vec<Pos2> {
        vec![self.outer[0], self.outer[1], self.inner[1], self.inner[0]]
    }

    /// The strut's two edges, and the round of the joint on the outside of its bend.
    pub(super) fn stroke(&self, painter: &Painter, stroke: Stroke, half: f32) {
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
pub(super) fn branch_figures(
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
pub(super) fn body_figures(
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
pub(super) fn clearance(
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
pub(super) fn base_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
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
pub(super) fn facets(painter: &Painter, view: &View, profile: &SupportProfile, column: &Column) {
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
