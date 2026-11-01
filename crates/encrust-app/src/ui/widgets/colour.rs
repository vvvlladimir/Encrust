//! A colour with its opacity: a swatch that opens a picker of a saturation and value square,
//! a hue strip, an opacity strip over a checkerboard, and the colour as `#RRGGBBAA`.

use egui::ecolor::Hsva;
use egui::{Color32, Id, Mesh, PopupCloseBehavior, Pos2, Rect, Sense, Ui, vec2};

use crate::i18n::tr;
use crate::ui::theme;
use crate::ui::widgets::{MENU_GAP, chooser_frame, text_box};

/// Points: the swatch, the square, and the strips under it.
const SWATCH: egui::Vec2 = vec2(56.0, theme::FIELD_H);
const SQUARE: egui::Vec2 = vec2(220.0, 150.0);
const STRIP_H: f32 = 14.0;
/// Points a checker square takes behind a colour that lets light through.
const CHECKER: f32 = 6.0;
/// Cells each way the square's gradient is cut into, so its colours are true at a glance.
const SQUARE_CELLS: usize = 16;
const HUE_CELLS: usize = 36;

/// A swatch of `rgba`, sRGB with straight alpha, which a click opens a picker on. Returns
/// whether the picker changed it.
pub fn colour_field(ui: &mut Ui, rgba: &mut [u8; 4], name: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(SWATCH, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ColorButton, true, name));
    paint_swatch(ui, rect, *rgba, response.hovered());
    super::focus_ring(ui, &response, theme::R_CONTROL);
    let hex = hex_of(*rgba);
    let response = response.on_hover_text(format!("{name} {hex}"));
    egui::Popup::menu(&response)
        .gap(MENU_GAP)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .frame(chooser_frame().inner_margin(theme::PANEL_PAD))
        .show(|ui| picker(ui, response.id, rgba))
        .is_some_and(|shown| shown.inner)
}

fn paint_swatch(ui: &Ui, rect: Rect, rgba: [u8; 4], hovered: bool) {
    let painter = ui.painter_at(rect);
    checker(&painter, rect);
    painter.rect_filled(rect, theme::R_CONTROL, theme::user_colour(rgba));
    let colors = theme::colors();
    let edge = if hovered {
        colors.text_low
    } else {
        colors.line
    };
    painter.rect_stroke(
        rect,
        theme::R_CONTROL,
        egui::Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
}

/// The picker's body. The hue is kept beside the colour, so a grey dragged out of the
/// square keeps the hue it was dragged from.
fn picker(ui: &mut Ui, id: Id, rgba: &mut [u8; 4]) -> bool {
    let kept = Id::new((id, "hsva"));
    let mut hsva = ui
        .data(|data| data.get_temp::<Hsva>(kept))
        .filter(|hsva| hsva.to_srgba_unmultiplied() == *rgba)
        .unwrap_or_else(|| Hsva::from_srgba_unmultiplied(*rgba));
    ui.spacing_mut().item_spacing = vec2(theme::ITEM_GAP, theme::ITEM_GAP);
    let mut changed = square(ui, &mut hsva);
    changed |= strip(
        ui,
        &mut hsva.h,
        |h| Hsva::new(h, 1.0, 1.0, 1.0).into(),
        false,
    );
    let opaque = Hsva { a: 1.0, ..hsva };
    let clear = Hsva { a: 0.0, ..hsva };
    changed |= strip(ui, &mut hsva.a, |a| Hsva { a, ..opaque }.into(), true) && clear != opaque;
    changed |= hex_row(ui, id, &mut hsva);
    ui.data_mut(|data| data.insert_temp(kept, hsva));
    if changed {
        *rgba = hsva.to_srgba_unmultiplied();
    }
    changed
}

/// Saturation across, value up, at the hue in hand.
fn square(ui: &mut Ui, hsva: &mut Hsva) -> bool {
    let (rect, response) = ui.allocate_exact_size(SQUARE, Sense::click_and_drag());
    let mut mesh = Mesh::default();
    let cells = SQUARE_CELLS as f32;
    for row in 0..=SQUARE_CELLS {
        for column in 0..=SQUARE_CELLS {
            let (s, v) = (column as f32 / cells, 1.0 - row as f32 / cells);
            let at = rect.min + vec2(s, 1.0 - v) * rect.size();
            mesh.colored_vertex(at, Hsva::new(hsva.h, s, v, 1.0).into());
        }
    }
    grid_faces(&mut mesh, SQUARE_CELLS, SQUARE_CELLS);
    ui.painter().add(mesh);
    let mark = rect.min + vec2(hsva.s, 1.0 - hsva.v) * rect.size();
    knob(ui, mark);
    let Some(at) = response.interact_pointer_pos() else {
        return false;
    };
    let share = ((at - rect.min) / rect.size()).clamp(vec2(0.0, 0.0), vec2(1.0, 1.0));
    hsva.s = share.x;
    hsva.v = 1.0 - share.y;
    true
}

/// A strip `value` slides along, 0.0 to 1.0, painted by `colour_at`; over a checkerboard
/// where it is the opacity.
fn strip(
    ui: &mut Ui,
    value: &mut f32,
    colour_at: impl Fn(f32) -> Color32,
    checkered: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(SQUARE.x, STRIP_H), Sense::click_and_drag());
    if checkered {
        checker(&ui.painter_at(rect), rect);
    }
    let mut mesh = Mesh::default();
    for step in 0..=HUE_CELLS {
        let share = step as f32 / HUE_CELLS as f32;
        let x = rect.left() + share * rect.width();
        let colour = colour_at(share);
        mesh.colored_vertex(Pos2::new(x, rect.top()), colour);
        mesh.colored_vertex(Pos2::new(x, rect.bottom()), colour);
    }
    grid_faces(&mut mesh, HUE_CELLS, 1);
    ui.painter().add(mesh);
    knob(
        ui,
        Pos2::new(rect.left() + *value * rect.width(), rect.center().y),
    );
    let Some(at) = response.interact_pointer_pos() else {
        return false;
    };
    *value = ((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    true
}

/// The colour as `#RRGGBBAA` to read or type, and its opacity as a share.
fn hex_row(ui: &mut Ui, id: Id, hsva: &mut Hsva) -> bool {
    let typed = Id::new((id, "hex"));
    let shown = hex_of(hsva.to_srgba_unmultiplied());
    let mut text = ui
        .data(|data| data.get_temp::<String>(typed))
        .unwrap_or_else(|| shown.clone());
    let mut changed = false;
    ui.horizontal(|ui| {
        let edit = text_box(egui::TextEdit::singleline(&mut text))
            .font(theme::figures(13.0))
            .background_color(theme::colors().sunken)
            .desired_width(SQUARE.x * 0.55);
        let response = ui.add(edit);
        if response.changed()
            && let Some(rgba) = parse_hex(&text)
        {
            *hsva = Hsva::from_srgba_unmultiplied(rgba);
            changed = true;
        }
        if !response.has_focus() {
            text = shown.clone();
        }
        let percent = (hsva.a * 100.0).round();
        ui.label(
            egui::RichText::new(tr!("colour-opacity", percent = percent))
                .color(theme::colors().text_mid),
        );
    });
    ui.data_mut(|data| data.insert_temp(typed, text));
    changed
}

fn knob(ui: &Ui, at: Pos2) {
    let colors = theme::colors();
    ui.painter()
        .circle_stroke(at, 5.0, egui::Stroke::new(3.0, colors.shadow));
    ui.painter()
        .circle_stroke(at, 5.0, egui::Stroke::new(1.5, colors.knob));
}

fn checker(painter: &egui::Painter, rect: Rect) {
    let colors = theme::colors();
    painter.rect_filled(rect, theme::R_CONTROL, colors.line);
    let (columns, rows) = (
        (rect.width() / CHECKER).ceil() as usize,
        (rect.height() / CHECKER).ceil() as usize,
    );
    for row in 0..rows {
        for column in (row % 2..columns).step_by(2) {
            let low = rect.min + vec2(column as f32, row as f32) * CHECKER;
            let cell = Rect::from_min_size(low, vec2(CHECKER, CHECKER)).intersect(rect);
            painter.rect_filled(cell, 0.0, colors.text_low);
        }
    }
}

/// Two triangles a cell of a mesh laid out row by row, `columns + 1` vertices a row.
fn grid_faces(mesh: &mut Mesh, columns: usize, rows: usize) {
    let width = columns as u32 + 1;
    for row in 0..rows as u32 {
        for column in 0..columns as u32 {
            let corner = row * width + column;
            mesh.add_triangle(corner, corner + 1, corner + width);
            mesh.add_triangle(corner + 1, corner + width + 1, corner + width);
        }
    }
}

/// `#RRGGBBAA`, upper case.
pub fn hex_of([r, g, b, a]: [u8; 4]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// Six or eight hex digits, a leading `#` optional; six are opaque.
pub fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let digits = text.trim().trim_start_matches('#');
    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    let alpha = if digits.len() == 8 { byte(6)? } else { u8::MAX };
    Some([byte(0)?, byte(2)?, byte(4)?, alpha])
}

#[cfg(test)]
mod tests;
