use super::column::{Column, lowest_trunk_mm};
use super::draw::{Pen, arc, dimension, plate_floor, revolved, text};
use super::view::View;
use super::{LANE, Part, mm};
use crate::i18n::tr;
use crate::ui::theme;
use egui::{Align2, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use printer_profiles::{RaftStyle, SupportProfile};

/// The raft rim on the plate, cut through one foot: thickness, wall lean, reach past the foot
/// and style. A raft switched off is drawn under the pad it would replace.
pub(super) fn raft_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let trunk = lowest_trunk_mm(profile);
    let column = if profile.raft.enabled {
        Column::on_raft(profile, 1, trunk)
    } else {
        Column::of(profile, 1, trunk)
    };
    let section = RaftSection::of(profile, column.pad);
    let rim = section.reach - section.pad;
    let along = (column.plate - section.tall * 2.5, column.plate);
    let across = ((section.pad - rim).max(0.0), section.reach + rim * 0.1);
    let view = View::standing(frame, along, across);
    plate_floor(painter, &view, column.plate);
    section.draw(painter, &view, &column, !profile.raft.enabled);
    revolved(painter, &view, &column.stations, 0.0, Part::Raft);
    raft_figures(painter, &view, profile, &column, &section);
}

/// A raft cut through one foot, millimetres: half its width at the supports and at the plate,
/// thickness, height, and half the foot's width.
pub(super) struct RaftSection {
    pub(super) top: f32,
    pub(super) bottom: f32,
    pub(super) thick: f32,
    pub(super) tall: f32,
    pub(super) reach: f32,
    pub(super) pad: f32,
    pub(super) style: RaftStyle,
}

impl RaftSection {
    pub(super) fn of(profile: &SupportProfile, pad: f32) -> Self {
        let raft = &profile.raft;
        let (thick, overhang) = (raft.thickness_mm, raft.overhang_mm());
        let top = pad + raft.margin_mm.max(0.0);
        let tall = match raft.style {
            RaftStyle::Skates => 2.0 * thick,
            RaftStyle::Grid | RaftStyle::Connected => thick,
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

    pub(super) fn draw(&self, painter: &Painter, view: &View, column: &Column, off: bool) {
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
    pub(super) fn slab(&self, view: &View, plate: f32, from: f32, to: f32) -> Vec<Pos2> {
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
    pub(super) fn pieces(&self, view: &View, plate: f32) -> Vec<Vec<Pos2>> {
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
    pub(super) fn saucer(&self, view: &View, plate: f32) -> Vec<Pos2> {
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
    pub(super) fn saucer_pieces(&self, view: &View, plate: f32) -> Vec<Vec<Pos2>> {
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

/// The slab's thickness, how far it reaches past the foot, and the lean of its wall.
pub(super) fn raft_figures(
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
        text(painter, at, Align2::CENTER_BOTTOM, &tr!("diagram-no-raft"));
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
