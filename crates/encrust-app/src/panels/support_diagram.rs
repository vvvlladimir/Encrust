//! The window a support's measurements are set on, each beside the part of a drawing of
//! the support it sizes. The drawing follows the numbers, so an edit shows as it is made.

use std::ops::RangeInclusive;

use core_supports::FOOT_BEVEL;
use egui::{Pos2, Rect, Sense, Shape, Stroke, Ui, UiBuilder, Vec2, pos2, vec2};
use printer_profiles::SupportProfile;

use crate::panels::Window;
use crate::panels::support_fields::keep_in_order;
use crate::supports::{Editing, Parameters};
use crate::ui::{Segment, Segmented, card, number_field, switch, theme};

/// The drawing's own size, points. The fields stand in its outer columns.
const REGULAR: Vec2 = vec2(460.0, 510.0);
const PILLAR: Vec2 = vec2(460.0, 330.0);
const FIELD_W: f32 = 64.0;

/// Points a millimetre of width is drawn at, and how far a millimetre of bite reaches.
/// Not to scale against the lengths: a tip half a millimetre across would not show.
const WIDTH_PX: f32 = 10.0;
const DEPTH_PX: f32 = 40.0;

/// Where the plate is drawn, and how far the left column of fields stands in.
const PLATE_Y: f32 = 480.0;
const LEFT_X: f32 = 8.0;
const RIGHT_X: f32 = 460.0 - 8.0 - FIELD_W;

pub fn ui(ctx: &egui::Context, window: &mut Window) {
    let Some(mut parameters) = window.tools.supports.parameters else {
        return;
    };
    let mut open = true;
    egui::Window::new("Support parameters")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .frame(card().inner_margin(theme::PANEL_MARGIN))
        .show(ctx, |ui| {
            let profile = match parameters.editing {
                Editing::Tool => Some(&mut window.tools.supports.profile),
                Editing::Settings => window
                    .machine
                    .settings
                    .support
                    .as_mut()
                    .map(|draft| &mut draft.values),
            };
            if let Some(profile) = profile {
                show(ui, &mut parameters, profile);
            }
        });
    window.tools.supports.parameters = open.then_some(parameters);
}

fn show(ui: &mut Ui, parameters: &mut Parameters, profile: &mut SupportProfile) {
    let segments = [
        Segment::new(false, "Regular support"),
        Segment::new(true, "Small pillar"),
    ];
    Segmented::new(&segments)
        .width(REGULAR.x)
        .show(ui, &mut parameters.small_pillar);
    ui.add_space(8.0);
    if parameters.small_pillar {
        small_pillar(ui, profile);
    } else {
        regular(ui, profile);
    }
    keep_in_order(profile);
}

/// One number set beside the point of the drawing it measures.
struct Callout<'a> {
    label: &'static str,
    value: &'a mut f32,
    step: f64,
    range: RangeInclusive<f32>,
    /// Top-left of the field in the drawing, and the point its leader runs to.
    field: Pos2,
    target: Pos2,
}

/// Two supports under the underside of a model: the left one carries the bite, the upper
/// width, the pillar and the foot's height, the right one the rest.
fn regular(ui: &mut Ui, profile: &mut SupportProfile) {
    let (canvas, _) = ui.allocate_exact_size(REGULAR, Sense::hover());
    let at = |point: Pos2| canvas.min + point.to_vec2();
    let scene = theme::scene();
    let painter = ui.painter_at(canvas);

    let model = [(90.0, 20.0), (370.0, 20.0), (290.0, 140.0), (170.0, 140.0)];
    let model = model.map(|(x, y)| at(pos2(x, y))).to_vec();
    painter.add(Shape::convex_polygon(model, scene.object, Stroke::NONE));
    let plate = Stroke::new(1.0, theme::colors().line);
    painter.hline(canvas.x_range(), canvas.top() + PLATE_Y, plate);

    let leg = Leg::of(profile);
    for mirror in [false, true] {
        for mut shape in leg.shapes(mirror, profile) {
            shape.translate(canvas.min.to_vec2());
            painter.add(shape);
        }
    }

    let callouts = [
        Callout {
            label: "Bite",
            value: &mut profile.tip.contact_depth_mm,
            step: 0.01,
            range: 0.05..=1.0,
            field: pos2(LEFT_X, 70.0),
            target: leg.stub_mid,
        },
        Callout {
            label: "Upper",
            value: &mut profile.top.upper_diameter_mm,
            step: 0.01,
            range: 0.05..=4.0,
            field: pos2(LEFT_X, 150.0),
            target: leg.upper_edge,
        },
        Callout {
            label: "Pillar",
            value: &mut profile.middle.diameter_mm,
            step: 0.05,
            range: 0.3..=6.0,
            field: pos2(LEFT_X, 260.0),
            target: leg.pillar_edge,
        },
        Callout {
            label: "Foot height",
            value: &mut profile.bottom.platform_thickness_mm,
            step: 0.05,
            range: 0.2..=5.0,
            field: pos2(LEFT_X, 400.0),
            target: leg.foot_edge,
        },
        Callout {
            label: "Contact",
            value: &mut profile.tip.contact_diameter_mm,
            step: 0.01,
            range: 0.05..=2.0,
            field: pos2(RIGHT_X, 70.0),
            target: mirrored(leg.contact),
        },
        Callout {
            label: "Length",
            value: &mut profile.top.length_mm,
            step: 0.05,
            range: 0.2..=10.0,
            field: pos2(RIGHT_X, 150.0),
            target: mirrored(leg.cone_mid),
        },
        Callout {
            label: "Lower",
            value: &mut profile.top.lower_diameter_mm,
            step: 0.05,
            range: 0.05..=6.0,
            field: pos2(RIGHT_X, 230.0),
            target: mirrored(leg.lower_edge),
        },
        Callout {
            label: "Flare upper",
            value: &mut profile.bottom.upper_diameter_mm,
            step: 0.05,
            range: 0.1..=10.0,
            field: pos2(RIGHT_X, 320.0),
            target: mirrored(leg.flare_top),
        },
        Callout {
            label: "Flare lower",
            value: &mut profile.bottom.lower_diameter_mm,
            step: 0.05,
            range: 0.1..=15.0,
            field: pos2(RIGHT_X, 400.0),
            target: mirrored(leg.flare_bottom),
        },
        Callout {
            label: "Foot",
            value: &mut profile.bottom.platform_diameter_mm,
            step: 0.05,
            range: 1.0..=30.0,
            field: pos2((REGULAR.x - FIELD_W) / 2.0, PLATE_Y - 50.0),
            target: leg.foot_corner,
        },
    ];
    for callout in callouts {
        place(ui, canvas, callout);
    }
}

/// The thin strut across the inside of a hollow, both ends sunk into its wall.
fn small_pillar(ui: &mut Ui, profile: &mut SupportProfile) {
    switch(
        ui,
        &mut profile.small_pillar.enabled,
        "Thin supports between two parts of the model",
    );
    ui.add_space(6.0);
    let (canvas, _) = ui.allocate_exact_size(PILLAR, Sense::hover());
    let at = |x: f32, y: f32| canvas.min + vec2(x, y);
    let scene = theme::scene();
    let painter = ui.painter_at(canvas);

    let square = Rect::from_min_max(at(100.0, 10.0), at(360.0, 310.0));
    painter.rect_filled(square, 0.0, scene.object);
    let centre = at(230.0, 160.0);
    painter.circle_filled(centre, 115.0, theme::colors().panel);

    let pillar = &profile.small_pillar;
    let width = (pillar.diameter_mm * WIDTH_PX).max(3.0);
    let (top, bend, bottom) = (at(200.0, 49.0), at(250.0, 160.0), at(200.0, 271.0));
    let upper = top + (top - bend).normalized() * (pillar.upper_depth_mm * DEPTH_PX).max(3.0);
    let lower = bottom + (bottom - bend).normalized() * (pillar.lower_depth_mm * DEPTH_PX).max(3.0);
    let colour = if pillar.enabled {
        theme::support_tint(0)
    } else {
        theme::colors().line
    };
    painter.add(Shape::line(
        vec![upper, bend, lower],
        Stroke::new(width, colour),
    ));

    let origin = canvas.min.to_vec2();
    let callouts = [
        Callout {
            label: "Upper depth",
            value: &mut profile.small_pillar.upper_depth_mm,
            step: 0.01,
            range: 0.05..=1.0,
            field: pos2(LEFT_X, 30.0),
            target: upper - origin,
        },
        Callout {
            label: "Lower depth",
            value: &mut profile.small_pillar.lower_depth_mm,
            step: 0.01,
            range: 0.05..=1.0,
            field: pos2(LEFT_X, 250.0),
            target: lower - origin,
        },
        Callout {
            label: "Diameter",
            value: &mut profile.small_pillar.diameter_mm,
            step: 0.01,
            range: 0.1..=3.0,
            field: pos2(RIGHT_X, 140.0),
            target: bend - origin,
        },
    ];
    for callout in callouts {
        place(ui, canvas, callout);
    }
}

/// The left support of the drawing in the drawing's own points, and the points its
/// callouts run to. The right one is its mirror.
struct Leg {
    contact: Pos2,
    base: Pos2,
    /// Across the cone, pointing out of the model's side.
    across: Vec2,
    direction: Vec2,
    widths: [f32; 4],
    foot: (f32, f32),
    /// Width where the flare leaves the pillar, where it meets the pad, and its rise.
    flare: (f32, f32, f32),
    stub_mid: Pos2,
    upper_edge: Pos2,
    cone_mid: Pos2,
    lower_edge: Pos2,
    pillar_edge: Pos2,
    flare_top: Pos2,
    flare_bottom: Pos2,
    foot_edge: Pos2,
    foot_corner: Pos2,
}

impl Leg {
    fn of(profile: &SupportProfile) -> Self {
        let contact = pos2(158.0, 122.0);
        let direction = vec2(-0.42, 1.0).normalized();
        let across = vec2(-direction.y, direction.x);
        let length = (profile.top.length_mm * 26.0).clamp(36.0, 110.0);
        let base = contact + direction * length;
        let widths = [
            (profile.tip.contact_diameter_mm * WIDTH_PX).max(2.0),
            (profile.top.upper_diameter_mm * WIDTH_PX).max(3.0),
            (profile.top.lower_diameter_mm * WIDTH_PX).max(4.0),
            (profile.middle.diameter_mm * WIDTH_PX).max(4.0),
        ];
        let foot_w = (profile.bottom.platform_diameter_mm * 9.0).clamp(24.0, 120.0);
        let foot_h = (profile.bottom.platform_thickness_mm * 16.0).clamp(8.0, 32.0);
        let bite = (profile.tip.contact_depth_mm * DEPTH_PX).clamp(3.0, 16.0);
        let foot_top = PLATE_Y - foot_h;

        // Both flare widths are drawn at the same points per millimetre, so half the
        // difference between them is the rise the mesh's forty-five degrees gives it.
        let flare_up = (profile.bottom.upper_diameter_mm * WIDTH_PX).max(widths[3]);
        let flare_down = (profile.bottom.lower_diameter_mm * WIDTH_PX).clamp(flare_up, foot_w);
        let rise = (flare_down - flare_up) / 2.0;
        let flare_y = foot_top - rise;

        Self {
            contact,
            base,
            across,
            direction,
            widths,
            foot: (foot_w, foot_h),
            flare: (flare_up, flare_down, rise),
            stub_mid: contact - direction * bite / 2.0,
            upper_edge: contact + direction * 4.0 + across * widths[1] / 2.0,
            cone_mid: contact + direction * length / 2.0 + across * widths[2] / 2.0,
            lower_edge: base + across * widths[2] / 2.0,
            pillar_edge: pos2(base.x - widths[3] / 2.0, (base.y + flare_y) / 2.0),
            flare_top: pos2(base.x - flare_up / 2.0, flare_y),
            flare_bottom: pos2(base.x - flare_down / 2.0, foot_top),
            foot_edge: pos2(base.x - foot_w / 2.0 + 4.0, foot_top + foot_h / 2.0),
            foot_corner: pos2(base.x + foot_w / 2.0 - foot_h * FOOT_BEVEL, PLATE_Y),
        }
    }

    /// The tip's bite, the cone, the pillar, the flare and the foot, in the drawing's own
    /// points.
    fn shapes(&self, mirror: bool, profile: &SupportProfile) -> Vec<Shape> {
        let flip = |point: Pos2| if mirror { mirrored(point) } else { point };
        let colour = theme::support_tint(0);
        let [contact_w, upper_w, lower_w, pillar_w] = self.widths;
        let bite = (profile.tip.contact_depth_mm * DEPTH_PX).clamp(3.0, 16.0);
        let quad = |from: Pos2, from_w: f32, to: Pos2, to_w: f32| {
            let across = self.across;
            let points = [
                from + across * from_w / 2.0,
                to + across * to_w / 2.0,
                to - across * to_w / 2.0,
                from - across * from_w / 2.0,
            ];
            Shape::convex_polygon(points.map(flip).to_vec(), colour, Stroke::NONE)
        };
        let (_, foot_h) = self.foot;
        let (flare_up, flare_down, rise) = self.flare;
        let foot_top = PLATE_Y - foot_h;
        let x = self.base.x;
        let pillar = Rect::from_two_pos(
            flip(pos2(x - pillar_w / 2.0, self.base.y)),
            flip(pos2(x + pillar_w / 2.0, foot_top - rise + 1.0)),
        );
        let mut shapes = vec![
            quad(
                self.contact - self.direction * bite,
                contact_w,
                self.contact,
                contact_w,
            ),
            quad(self.contact, upper_w, self.base, lower_w),
            Shape::circle_filled(flip(self.base), lower_w.max(pillar_w) / 2.0, colour),
            Shape::rect_filled(pillar, 0.0, colour),
            self.band(mirror, foot_top - rise, flare_up, foot_top, flare_down),
        ];
        shapes.extend(self.pad(mirror, profile));
        shapes
    }

    /// The pad, as the mesh sweeps it: one trapezoid in to the rim it meets the plate on,
    /// and a narrowed top edge when the foot tapers towards the support it carries.
    fn pad(&self, mirror: bool, profile: &SupportProfile) -> Vec<Shape> {
        let (foot_w, foot_h) = self.foot;
        let foot_top = PLATE_Y - foot_h;
        let bevel = foot_h * FOOT_BEVEL;
        // A thick foot of a small diameter would bevel past its own axis and turn the
        // trapezoid inside out; the mesh stops at the trunk for the same reason.
        let plate_w = (foot_w - bevel * 2.0).max(self.widths[3]);
        if !profile.bottom.shape.tapers() {
            return vec![self.band(mirror, foot_top, foot_w, PLATE_Y, plate_w)];
        }

        let top_w = (foot_w * 0.55).max(self.flare.1);
        vec![
            self.band(mirror, foot_top, top_w, PLATE_Y - bevel, foot_w),
            self.band(mirror, PLATE_Y - bevel, foot_w, PLATE_Y, plate_w),
        ]
    }

    /// One band of the foot about the support's own axis, `top_w` wide at `top` and
    /// `bottom_w` wide at `bottom`.
    fn band(&self, mirror: bool, top: f32, top_w: f32, bottom: f32, bottom_w: f32) -> Shape {
        let x = self.base.x;
        let points = [
            pos2(x - top_w / 2.0, top),
            pos2(x + top_w / 2.0, top),
            pos2(x + bottom_w / 2.0, bottom),
            pos2(x - bottom_w / 2.0, bottom),
        ];
        let points = points.map(|point| if mirror { mirrored(point) } else { point });
        Shape::convex_polygon(points.to_vec(), theme::support_tint(0), Stroke::NONE)
    }
}

fn mirrored(point: Pos2) -> Pos2 {
    pos2(REGULAR.x - point.x, point.y)
}

/// Draws a callout's label, its field and the leader from the field to its point.
fn place(ui: &mut Ui, canvas: Rect, callout: Callout<'_>) {
    let colors = theme::colors();
    let field = Rect::from_min_size(
        canvas.min + callout.field.to_vec2(),
        vec2(FIELD_W, theme::FIELD_H),
    );
    let target = canvas.min + callout.target.to_vec2();
    let on_left = field.center().x < canvas.center().x;
    let (anchor, elbow) = if on_left {
        (field.right_center(), field.right_center() + vec2(14.0, 0.0))
    } else {
        (field.left_center(), field.left_center() - vec2(14.0, 0.0))
    };

    let painter = ui.painter();
    let leader = Stroke::new(1.0, colors.text_low);
    painter.line_segment([anchor + vec2(0.0, -5.0), anchor + vec2(0.0, 5.0)], leader);
    painter.add(Shape::line(vec![anchor, elbow, target], leader));
    painter.circle_filled(target, 2.0, colors.text_mid);
    painter.text(
        field.left_top() - vec2(0.0, 2.0),
        egui::Align2::LEFT_BOTTOM,
        callout.label,
        theme::small(),
        colors.text_low,
    );

    ui.scope_builder(UiBuilder::new().max_rect(field), |ui| {
        number_field(
            ui,
            callout.value,
            callout.step,
            callout.range,
            Some(2),
            FIELD_W,
        );
    });
}
