use super::column::Column;
use super::draw::{
    Pen, accent_line, dimension, label_width, plate_floor, plate_wall, revolved, text,
};
use super::view::View;
use super::{
    BRACE_LABEL_W, BRACE_MIDDLE_LEVELS, FIGURE_H, LANE, LANE_STEP, Part, WALL_W, diameter, mm,
};
use crate::i18n::tr;
use crate::ui::theme;
use core_supports::{BraceBand, BraceLevel, brace_levels};
use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};
use printer_profiles::{BracePattern, Bracing, SupportProfile};

/// Two trunks the spacing apart standing on the plate, the braces tying them zone by zone
/// as the generator places them, and where they go.
pub(super) fn brace_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let bracing = &profile.bracing;
    let gap = bracing.max_spacing_mm / 2.0;
    let rise = bracing.rise_mm.max(0.1);
    let shown = bracing.bottom.levels + BRACE_MIDDLE_LEVELS + bracing.top.levels;
    let ceiling = bracing.start_height_mm + rise * shown as f32;
    // A head of trunk above the top level, where it would split into its branches.
    let column = Column::of(profile, 1, ceiling + rise / 2.0);
    let shown = Bracing {
        rise_mm: rise,
        ..*bracing
    };
    let levels = brace_levels(&shown, bracing.start_height_mm, ceiling);
    let along = |z: f32| column.plate - z;
    let half = gap + column.base.max(profile.pillar_radius_mm());
    let view = brace_frame(frame, (along(ceiling), column.plate), half);
    if view.standing {
        plate_floor(painter, &view, column.plate);
    } else {
        plate_wall(painter, &view, column.plate);
    }
    for side in [-1.0, 1.0] {
        revolved(painter, &view, &column.stations, side * gap, Part::Brace);
    }
    for level in &levels {
        let (near, far) = (along(level.z_mm), along(level.z_mm + rise));
        let thick = brace_half(profile, &column, near, far);
        let lean = if level.flipped { -gap } else { gap };
        let mut ties = vec![(-lean, lean)];
        if level.pattern == BracePattern::Cross {
            ties.push((lean, -lean));
        }
        for (from, to) in ties {
            strut(
                painter,
                &view,
                (near, from),
                (far, to),
                thick,
                !bracing.enabled,
            );
        }
    }
    brace_figures(painter, &view, profile, &column, &levels, gap);
}

/// The braces lying or standing, whichever shows `along` and `half` either side of the
/// axis the larger, with room before `along` for the figures written there.
pub(super) fn brace_frame(frame: Rect, along: (f32, f32), half: f32) -> View {
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
pub(super) fn brace_half(profile: &SupportProfile, column: &Column, near: f32, far: f32) -> f32 {
    let bracing = profile.bracing.radius_mm();
    bracing.min(column.half_at(near)).min(column.half_at(far))
}

/// One brace from `from` to `to`, millimetres, `thick` either side of its line.
pub(super) fn strut(
    painter: &Painter,
    view: &View,
    from: (f32, f32),
    to: (f32, f32),
    thick: f32,
    off: bool,
) {
    let (a, b) = (view.at(from.0, from.1), view.at(to.0, to.1));
    let side = (b - a).normalized().rot90() * view.len(thick);
    let outline = vec![a - side, b - side, b + side, a + side];
    painter.extend(Pen::Picked.outline(outline, off));
}

/// Where the first brace goes, the level spacing on either side of the pair, each level's
/// zone beside the far trunk, and above them the trunk spacing and brace width.
pub(super) fn brace_figures(
    painter: &Painter,
    view: &View,
    profile: &SupportProfile,
    column: &Column,
    levels: &[BraceLevel],
    gap: f32,
) {
    let bracing = &profile.bracing;
    let rise = bracing.rise_mm.max(0.1);
    let at_z = |z: f32, y: f32| view.at(column.plate - z, y);
    let (along, across) = (view.along_dir(), view.across_dir());
    let first = bracing.start_height_mm;
    let clear = across * (view.len(column.half_at(column.plate - first)) + LANE);

    let (from, to) = (at_z(first, -gap) - clear, at_z(first + rise, -gap) - clear);
    dimension(painter, from, to, "");
    let every = tr!("diagram-every", value = mm(bracing.rise_mm));
    beside(painter, from.lerp(to, 0.5), -across, &every);

    let (from, to) = (at_z(0.0, gap) + clear, at_z(first, gap) + clear);
    dimension(painter, from, to, "");
    let start = tr!("diagram-first", value = mm(bracing.start_height_mm));
    beside(painter, from.lerp(to, 0.5), across, &start);
    let lane = clear + across * LANE_STEP;
    zone_marks(painter, view, levels, rise, |z| at_z(z, gap) + lane);

    let top = levels
        .last()
        .map_or(first + rise, |level| level.z_mm + rise);
    let back = along * 8.0;
    let (from, to) = (at_z(top, -gap) - back, at_z(top, gap) - back);
    dimension(painter, from, to, "");
    let spacing = format!("≤{:.1}", bracing.max_spacing_mm);
    beside(painter, from.lerp(to, 0.5), -along, &spacing);
    if let Some(level) = levels.last() {
        // Where the strut leaving the near trunk is two fifths of the way across.
        let lean = if level.flipped { -gap } else { gap };
        let t = (lean - 0.4 * gap) / (2.0 * lean);
        let on = at_z(level.z_mm, -lean).lerp(at_z(level.z_mm + rise, lean), t);
        let label = diameter(bracing.diameter_mm);
        let room = across * (label_width(painter, &label) / 2.0 + 4.0);
        let out = at_z(level.z_mm + t * rise, -gap) - clear - room;
        let leader = Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.5));
        painter.line_segment([on, out + room], leader);
        text(painter, out, Align2::CENTER_CENTER, &label);
    }
    if !bracing.enabled {
        let at = at_z(top, -gap) - clear;
        beside(painter, at, -across, &tr!("diagram-no-bracing"));
    }
}

/// A bracket beside the far trunk over each zone's levels, named for the zone, a lane
/// further out than the start's figure so the two never meet.
pub(super) fn zone_marks(
    painter: &Painter,
    view: &View,
    levels: &[BraceLevel],
    rise: f32,
    at: impl Fn(f32) -> Pos2,
) {
    let across = view.across_dir();
    for zone in levels.chunk_by(|a, b| a.band == b.band) {
        let (Some(low), Some(high)) = (zone.first(), zone.last()) else {
            continue;
        };
        let (from, to) = (at(low.z_mm), at(high.z_mm + rise));
        let ends = across * 3.0;
        let stroke = accent_line();
        painter.line_segment([from, to], stroke);
        painter.line_segment([from - ends, from + ends], stroke);
        painter.line_segment([to - ends, to + ends], stroke);
        let name = match low.band {
            BraceBand::Bottom => tr!("diagram-brace-bottom"),
            BraceBand::Middle => tr!("diagram-brace-middle"),
            BraceBand::Top => tr!("diagram-brace-top"),
        };
        beside(painter, from.lerp(to, 0.5), across, &name);
    }
}

/// A figure written next to `at`, on its `out` side and clear of it.
pub(super) fn beside(painter: &Painter, at: Pos2, out: Vec2, label: &str) {
    let width = label_width(painter, label);
    let reach = out.x.abs() * (width / 2.0 + 4.0) + out.y.abs() * (FIGURE_H / 2.0 + 2.0);
    text(painter, at + out * reach, Align2::CENTER_CENTER, label);
}
