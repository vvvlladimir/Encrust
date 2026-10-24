use super::column::{Column, lowest_trunk_mm};
use super::draw::{across, dimension, model_wall, revolved, text};
use super::view::View;
use super::{ARROW, Part, TIP_LABEL_W, diameter, mm};
use crate::ui::theme;
use egui::{Align2, Painter, Rect, Stroke, pos2};
use printer_profiles::{ContactShape, SupportProfile};

/// The tip close up: the contact, how far it is pushed in, and the cone's length.
pub(super) fn tip_view(painter: &Painter, frame: Rect, profile: &SupportProfile) {
    let column = Column::of(profile, 1, lowest_trunk_mm(profile));
    let (bite, length) = (profile.tip.contact_depth_mm, profile.top.length_mm);
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
pub(super) fn tip_figures(painter: &Painter, view: &View, profile: &SupportProfile, ball: f32) {
    let (bite, length) = (profile.tip.contact_depth_mm, profile.top.length_mm);
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
pub(super) fn contact_centre(profile: &SupportProfile) -> f32 {
    profile.contact_radius_mm() - profile.tip.contact_depth_mm
}
