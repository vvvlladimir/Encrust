//! The sheet that lists every key, drawn from `crate::shortcuts` alone.

use egui::{Align, Align2, Layout, RichText, Sense, StrokeKind, vec2};

use crate::i18n::tr;
use crate::shortcuts::{BINDINGS, GESTURES, Group, chord_keys};
use crate::ui::{dialog, dialog_head, group_name, hairline, icon, theme};

/// The groups down each column, split so the two columns come out about even.
const LEFT: [Group; 2] = [Group::Tools, Group::Plate];
const RIGHT: [Group; 3] = [Group::Window, Group::Layers, Group::Files];

/// Two columns and the gap between them fill the sheet's 800 inside its margin.
const COLUMN_W: f32 = 364.0;
const COLUMN_GAP: f32 = 28.0;
const ROW_H: f32 = 30.0;
/// Points between the keys of one chord, and either side of the "or" between two chords.
const KEY_GAP: f32 = 3.0;
const OR_GAP: f32 = 6.0;
/// What the sheet leaves of the window's height for its head, its foot and its margin.
const CHROME_H: f32 = 200.0;

/// Padding inside a keycap, and the smallest one a single character gets.
const CAP_PAD: egui::Vec2 = vec2(7.0, 3.0);
const CAP_MIN_W: f32 = 22.0;

/// Shows the sheet over the window, and closes it on Escape or a click outside.
pub fn ui(ctx: &egui::Context, open: &mut bool) {
    let sheet = dialog(egui::Id::new("shortcuts"), 18).show(ctx, |ui| {
        ui.set_width(COLUMN_W * 2.0 + COLUMN_GAP);
        header(ui, open);
        ui.add_space(10.0);
        let room = (ui.ctx().content_rect().height() - CHROME_H).max(ROW_H * 4.0);
        egui::ScrollArea::vertical()
            .max_height(room)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    column(ui, &LEFT);
                    ui.add_space(COLUMN_GAP);
                    column(ui, &RIGHT);
                });
            });
        ui.add_space(14.0);
        hairline(ui);
        ui.add_space(8.0);
        ui.label(
            RichText::new(tr!("shortcuts-note"))
                .font(theme::small())
                .color(theme::colors().text_low),
        );
    });
    if sheet.should_close() {
        *open = false;
    }
}

fn header(ui: &mut egui::Ui, open: &mut bool) {
    if dialog_head(ui, icon::KEYBOARD, &tr!("shortcuts-title")) {
        *open = false;
    }
    ui.add_space(8.0);
    hairline(ui);
}

fn column(ui: &mut egui::Ui, groups: &[Group]) {
    ui.allocate_ui_with_layout(vec2(COLUMN_W, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(COLUMN_W);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                ui.add_space(12.0);
            }
            group_name(ui, &group.label());
            for binding in BINDINGS.iter().filter(|binding| binding.group == *group) {
                let chords: Vec<Vec<&str>> = binding.keys.iter().map(chord_keys).collect();
                row(ui, &binding.action.label(), &chords);
            }
            for gesture in GESTURES.iter().filter(|gesture| gesture.group == *group) {
                let keys = (gesture.keys)();
                row(ui, &(gesture.label)(), &[vec![keys.as_str()]]);
            }
        }
    });
}

/// What the keys do on the left, the keycaps themselves on the right: one cap a key, and
/// "or" between two chords that do the same.
fn row(ui: &mut egui::Ui, label: &str, chords: &[Vec<&str>]) {
    let (rect, _) = ui.allocate_exact_size(vec2(COLUMN_W, ROW_H), Sense::hover());
    let colors = theme::colors();
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::label(),
        colors.text_mid,
    );

    let mut right = rect.right();
    for (index, chord) in chords.iter().rev().enumerate() {
        if index > 0 {
            right = or(ui, right - OR_GAP, rect.center().y) - OR_GAP;
        }
        for key in chord.iter().rev() {
            right = keycap(ui, key, right, rect.center().y) - KEY_GAP;
        }
        right += KEY_GAP;
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, colors.hairline),
    );
}

/// The word between two chords, drawn from its right edge leftwards. Returns its left.
fn or(ui: &egui::Ui, right: f32, center_y: f32) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        tr!("shortcuts-or"),
        theme::small(),
        theme::colors().text_low,
    );
    let left = right - galley.size().x;
    let at = egui::pos2(left, center_y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, theme::colors().text_low);
    left
}

/// One keycap, drawn from its right edge leftwards. Returns the edge it ended on, so the
/// next cap of the same row knows where to go.
fn keycap(ui: &egui::Ui, text: &str, right: f32, center_y: f32) -> f32 {
    let colors = theme::colors();
    let font = theme::figures(12.0);
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
