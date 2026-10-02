use egui::{Align2, Frame, RichText, Sense, Ui, vec2};

use crate::ui::theme;

/// Where `describe` sends its text while a section's body is being drawn.
const DESCRIBING: &str = "encrust-describing";

/// How far what a switch reveals is set in under it, points.
const NEST_INDENT: i8 = 14;

/// One block of the inspector: a heading, an optional hint on the right, the body, and a
/// hairline under the lot. What the body `describe`s opens from a click on the title.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    hint_text: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    block(ui, title, hint_text, None, add).1
}

/// A block like `section` with one icon button at the right of its heading, for the
/// action that belongs to the whole block. Returns whether it was pressed.
pub fn section_with_action<R>(
    ui: &mut Ui,
    title: &str,
    action: (&str, &str),
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, R) {
    block(ui, title, None, Some(action), add)
}

fn block<R>(
    ui: &mut Ui,
    title: &str,
    hint_text: Option<&str>,
    action: Option<(&str, &str)>,
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, R) {
    let id = ui.id().with(("section", title));
    let inner = Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            let (title_response, pressed) = ui
                .horizontal(|ui| {
                    let title_response = block_title(ui, id, title);
                    let layout = egui::Layout::right_to_left(egui::Align::Center);
                    let pressed = ui.with_layout(layout, |ui| {
                        let pressed = action.is_some_and(|(glyph, tooltip)| {
                            crate::ui::icon_button(ui, glyph, tooltip).clicked()
                        });
                        if let Some(hint_text) = hint_text {
                            hint(ui, hint_text);
                        }
                        pressed
                    });
                    (title_response, pressed.inner)
                })
                .inner;
            ui.add_space(3.0);
            ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
            let (inner, texts) = collecting(ui, id, add);
            show_description(ui, id, &title_response, texts);
            (pressed, inner)
        })
        .inner;
    hairline(ui);
    inner
}

/// The title of a block. It answers a click, and says so with a glyph, once the body has
/// described itself.
fn block_title(ui: &mut Ui, id: egui::Id, title: &str) -> egui::Response {
    let described = ui.data(|data| data.get_temp::<bool>(id.with("described"))) == Some(true);
    let colors = theme::colors();
    let mut text = egui::text::LayoutJob::default();
    let format = |font, color| egui::TextFormat::simple(font, color);
    text.append(title, 0.0, format(theme::section(), colors.text_mid));
    if described {
        let glyph = format(theme::icon(12.0), colors.text_low);
        text.append(crate::ui::icon::INFO, 6.0, glyph);
    }
    let sense = if described {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.add(egui::Label::new(text).sense(sense).selectable(false));
    if described {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Runs `add` with `describe` pointed at this block, and returns what it described.
fn collecting<R>(ui: &mut Ui, id: egui::Id, add: impl FnOnce(&mut Ui) -> R) -> (R, Vec<String>) {
    let key = egui::Id::new(DESCRIBING);
    let outer = ui.data_mut(|data| {
        data.insert_temp(id.with("texts"), Vec::<String>::new());
        let outer = data.get_temp::<egui::Id>(key);
        data.insert_temp(key, id);
        outer
    });
    let inner = add(ui);
    let texts = ui.data_mut(|data| {
        match outer {
            Some(outer) => {
                data.insert_temp(key, outer);
            }
            None => {
                data.remove::<egui::Id>(key);
            }
        }
        data.remove_temp::<Vec<String>>(id.with("texts"))
            .unwrap_or_default()
    });
    (inner, texts)
}

/// Opens and closes the block's description under its title, and draws it while open.
fn show_description(ui: &Ui, id: egui::Id, title: &egui::Response, texts: Vec<String>) {
    let open_id = id.with("open");
    let described_id = id.with("described");
    let was_open = ui.data(|data| data.get_temp::<bool>(open_id)) == Some(true);
    let elsewhere = ui.input(|input| input.pointer.any_click()) && !title.clicked();
    let open = !texts.is_empty() && (was_open ^ title.clicked()) && !(was_open && elsewhere);
    ui.data_mut(|data| {
        data.insert_temp(open_id, open);
        data.insert_temp(described_id, !texts.is_empty());
    });
    if !open {
        return;
    }
    let width = ui.available_width().max(160.0);
    egui::Area::new(id.with("description"))
        .order(egui::Order::Foreground)
        .fixed_pos(title.rect.left_bottom() + vec2(0.0, 6.0))
        .show(ui.ctx(), |ui| {
            card().inner_margin(theme::CARD_MARGIN).show(ui, |ui| {
                ui.set_max_width(width);
                ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                for text in &texts {
                    ui.label(
                        RichText::new(text)
                            .font(theme::small())
                            .color(theme::colors().text_mid),
                    );
                }
            });
        });
}

/// What a block is for, or what a setting in it does: kept out of the way, and shown
/// under the block's title when the title is clicked. Outside a block, a plain `hint`.
pub fn describe(ui: &mut Ui, text: &str) {
    let key = egui::Id::new(DESCRIBING);
    let target = ui.data(|data| data.get_temp::<egui::Id>(key));
    match target {
        Some(id) => ui.data_mut(|data| {
            data.get_temp_mut_or_default::<Vec<String>>(id.with("texts"))
                .push(text.to_owned());
        }),
        None => hint(ui, text),
    }
}

/// What a switch reveals, set in under it with a rule down its left edge, so it reads as
/// that switch's own.
pub fn nested<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let margin = egui::Margin {
        left: NEST_INDENT,
        ..egui::Margin::ZERO
    };
    let shown = Frame::new().inner_margin(margin).show(ui, add);
    let rect = shown.response.rect;
    let rule = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 3.0, rect.top()),
        egui::pos2(rect.left() + 5.0, rect.bottom()),
    );
    ui.painter()
        .rect_filled(rule, theme::R_CONTROL, theme::colors().line);
    shown.inner
}

/// The frame of anything that floats over the viewport.
pub fn card() -> Frame {
    Frame::new()
        .fill(theme::colors().panel)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_SURFACE)
        .shadow(theme::shadow())
}

/// A group of fields under a title that folds them away, so a long form reads as its
/// headings. Starts folded; which are open is kept per title.
pub fn fold(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let id = ui.id().with(("fold", title));
    let open = ui.data(|data| data.get_temp::<bool>(id)) == Some(true);
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.hover);
    }
    let glyph = if open {
        crate::ui::icon::CARET_DOWN
    } else {
        crate::ui::icon::CARET_RIGHT
    };
    let left = rect.left_center() + vec2(6.0, 0.0);
    let painter = ui.painter();
    painter.text(
        left,
        Align2::LEFT_CENTER,
        glyph,
        theme::icon(12.0),
        colors.text_low,
    );
    let title_at = left + vec2(18.0, 0.0);
    painter.text(
        title_at,
        Align2::LEFT_CENTER,
        title,
        theme::section(),
        colors.text_mid,
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {
        ui.data_mut(|data| data.insert_temp(id, !open));
    }
    if open {
        ui.add_space(2.0);
        add(ui);
        ui.add_space(6.0);
    }
}

/// A section heading, used by the inspector and by the header row of a floating card.
pub fn heading(ui: &mut Ui, title: &str, hint_text: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::section())
                .color(theme::colors().text_mid),
        );
        if let Some(hint_text) = hint_text {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                hint(ui, hint_text);
            });
        }
    });
}

/// A heading inside a block: the group of fields under it belongs together.
pub fn subheading(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(title)
            .font(theme::small())
            .color(theme::colors().text_low),
    );
    ui.add_space(2.0);
}

/// A quiet aside: a unit, a count, a reason something is greyed out.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme::small())
            .color(theme::colors().text_low),
    );
}

/// Lines of numbers under a picker: the build volume, the exposure the resin asks for.
pub fn meta(ui: &mut Ui, lines: &[String]) {
    for line in lines {
        ui.label(
            RichText::new(line)
                .font(theme::mono(11.0))
                .color(theme::colors().text_low),
        );
    }
}

/// A grid of two columns of measured values. The cells are the panel colour, so the
/// hairline showing between them is the whole of the grid.
pub fn stats(ui: &mut Ui, entries: &[(&str, String)]) {
    let cell_width = (ui.available_width() - 1.0) / 2.0;
    Frame::new()
        .fill(theme::colors().hairline)
        .corner_radius(theme::R_SURFACE)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
            for pair in entries.chunks(2) {
                ui.horizontal(|ui| {
                    for (key, value) in pair {
                        cell(ui, cell_width, key, value);
                    }
                });
            }
        });
}

fn cell(ui: &mut Ui, width: f32, key: &str, value: &str) {
    Frame::new()
        .fill(theme::colors().panel)
        .inner_margin(theme::CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width((width - theme::CARD_MARGIN.sum().x).max(0.0));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    RichText::new(key)
                        .font(theme::small())
                        .color(theme::colors().text_low),
                );
                ui.label(
                    RichText::new(value)
                        .font(theme::mono(13.0))
                        .color(theme::colors().text_high),
                );
            });
        });
}

/// Rows of one reading each, the name at the left and the value at the right, for a
/// summary read top to bottom rather than compared across.
pub fn readings(ui: &mut Ui, entries: &[(&str, String)]) {
    let colors = theme::colors();
    for (key, value) in entries {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::hover());
        let painter = ui.painter();
        painter.text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            key,
            theme::label(),
            colors.text_mid,
        );
        painter.text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            value,
            theme::mono(12.0),
            colors.text_high,
        );
    }
}

/// A one pixel divider across the full width of whatever is drawing it.
pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}
