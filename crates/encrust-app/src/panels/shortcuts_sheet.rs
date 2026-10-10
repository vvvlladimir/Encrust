//! The sheet that lists every key, drawn from `crate::shortcuts` alone.

use egui::{Align, Align2, Layout, RichText, Sense, StrokeKind, vec2};

use crate::shortcuts::{BINDINGS, GESTURES, Group, chord_text};
use crate::ui::{hairline, icon, icon_button, theme};

/// The groups down each column, split so the two columns come out about even.
const LEFT: [Group; 2] = [Group::Tools, Group::Plate];
const RIGHT: [Group; 3] = [Group::Window, Group::Layers, Group::Files];

const COLUMN_W: f32 = 268.0;
const COLUMN_GAP: f32 = 32.0;
const ROW_H: f32 = 24.0;

/// Padding inside a keycap, and the smallest one a single character gets.
const CAP_PAD: egui::Vec2 = vec2(7.0, 3.0);
const CAP_MIN_W: f32 = 22.0;

/// Shows the sheet over the window, and closes it on Escape or a click outside.
pub fn ui(ctx: &egui::Context, open: &mut bool) {
    let sheet = egui::Modal::new(egui::Id::new("shortcuts"))
        .frame(
            egui::Frame::new()
                .fill(theme::colors().panel)
                .corner_radius(theme::R_SURFACE)
                .inner_margin(egui::Margin::same(18))
                .shadow(theme::shadow()),
        )
        .show(ctx, |ui| {
            ui.set_width(COLUMN_W * 2.0 + COLUMN_GAP);
            header(ui, open);
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                column(ui, &LEFT);
                ui.add_space(COLUMN_GAP);
                column(ui, &RIGHT);
            });
            ui.add_space(14.0);
            hairline(ui);
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "A tool takes the digit of its place on the rail. Keys are off while a \
                     field has the cursor.",
                )
                .font(theme::small())
                .color(theme::colors().text_low),
            );
        });
    if sheet.should_close() {
        *open = false;
    }
}

fn header(ui: &mut egui::Ui, open: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Keyboard shortcuts")
                .font(theme::body())
                .color(theme::colors().text_high),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, icon::CANCEL, "Close").clicked() {
                *open = false;
            }
        });
    });
    ui.add_space(8.0);
    hairline(ui);
}

fn column(ui: &mut egui::Ui, groups: &[Group]) {
    ui.allocate_ui_with_layout(vec2(COLUMN_W, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(COLUMN_W);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                ui.add_space(14.0);
            }
            ui.label(
                RichText::new(group.label())
                    .font(theme::small())
                    .color(theme::colors().text_low),
            );
            ui.add_space(4.0);
            for binding in BINDINGS.iter().filter(|binding| binding.group == *group) {
                let caps: Vec<String> = binding.keys.iter().map(chord_text).collect();
                row(ui, binding.action.label(), &caps);
            }
            for gesture in GESTURES.iter().filter(|gesture| gesture.group == *group) {
                row(ui, gesture.label, &[gesture.keys.to_owned()]);
            }
        }
    });
}

/// What the keys do on the left, the keycaps themselves on the right.
fn row(ui: &mut egui::Ui, label: &str, caps: &[String]) {
    let (rect, _) = ui.allocate_exact_size(vec2(COLUMN_W, ROW_H), Sense::hover());
    let colors = theme::colors();
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::label(),
        colors.text_high,
    );

    let mut right = rect.right();
    for cap in caps.iter().rev() {
        right = keycap(ui, cap, right, rect.center().y) - 5.0;
    }
}

/// One keycap, drawn from its right edge leftwards. Returns the edge it ended on, so the
/// next cap of the same row knows where to go.
fn keycap(ui: &egui::Ui, text: &str, right: f32, center_y: f32) -> f32 {
    let colors = theme::colors();
    let font = theme::figures(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, colors.text_mid);
    let width = (galley.size().x + CAP_PAD.x * 2.0).max(CAP_MIN_W);
    let height = galley.size().y + CAP_PAD.y * 2.0;
    let cap = egui::Rect::from_min_size(
        egui::pos2(right - width, center_y - height / 2.0),
        vec2(width, height),
    );

    ui.painter().rect(
        cap,
        theme::R_CONTROL,
        colors.raised,
        egui::Stroke::new(1.0, colors.line),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(
            cap.center().x - galley.size().x / 2.0,
            cap.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.text_mid,
    );
    cap.left()
}
