use egui::{Align2, Frame, RichText, Sense, Ui, UiBuilder, vec2};

use crate::ui::theme;

/// Where `describe` sends its text while a section's body is being drawn.
const DESCRIBING: &str = "encrust-describing";

/// How far what a switch reveals is set in under it, points.
const NEST_INDENT: i8 = 14;

/// One block of the inspector: a heading that folds it, an optional hint on the right,
/// the body, and a hairline under the lot. What the body `describe`s is a question mark
/// beside the title, read on hover.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    hint_text: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    block(ui, title, hint_text, None, add).1
}

/// A block like `section` with one icon button at the right of its heading, for the
/// action that belongs to the whole block. Returns whether it was pressed.
pub fn section_with_action<R>(
    ui: &mut Ui,
    title: &str,
    action: (&str, &str),
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, Option<R>) {
    block(ui, title, None, Some(action), add)
}

/// Body margin of a block: the heading row carries its own top, so the body has none.
const BODY_MARGIN: egui::Margin = egui::Margin {
    left: theme::PANEL_MARGIN.left,
    right: theme::PANEL_MARGIN.right,
    top: 0,
    bottom: theme::PANEL_MARGIN.bottom,
};

fn block<R>(
    ui: &mut Ui,
    title: &str,
    hint_text: Option<&str>,
    action: Option<(&str, &str)>,
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, Option<R>) {
    let id = ui.id().with(("section", title));
    let open = ui.data(|data| data.get_temp::<bool>(id.with("open"))) != Some(false);
    let pressed = block_heading(ui, id, title, open, hint_text, action);
    let inner = open.then(|| {
        Frame::new()
            .inner_margin(BODY_MARGIN)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                let (inner, texts) = collecting(ui, id, add);
                ui.data_mut(|data| data.insert_temp(id.with("texts"), texts));
                inner
            })
            .inner
    });
    hairline(ui);
    (pressed, inner)
}

/// The heading row of a block: caret and title, which fold it, the question mark that
/// reads out what the body described, and the hint and action at the right.
fn block_heading(
    ui: &mut Ui,
    id: egui::Id,
    title: &str,
    open: bool,
    hint_text: Option<&str>,
    action: Option<(&str, &str)>,
) -> bool {
    let colors = theme::colors();
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::SECTION_H), Sense::hover());
    let inner = rect.shrink2(vec2(theme::PANEL_PAD - 4.0, 0.0));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(layout));

    let caret = if open {
        crate::ui::icon::CARET_DOWN
    } else {
        crate::ui::icon::CARET_RIGHT
    };
    let mut text = egui::text::LayoutJob::default();
    let format = |font, color| egui::TextFormat::simple(font, color);
    text.append(caret, 0.0, format(theme::icon(11.0), colors.text_mid));
    text.append(title, 6.0, format(theme::section(), colors.text_high));
    let fold = child.add(
        egui::Label::new(text)
            .sense(Sense::click())
            .selectable(false),
    );
    if fold
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        child.data_mut(|data| data.insert_temp(id.with("open"), !open));
    }

    let texts = child.data(|data| data.get_temp::<Vec<String>>(id.with("texts")));
    if let Some(texts) = texts.filter(|texts| !texts.is_empty()) {
        let glyph = RichText::new(crate::ui::icon::QUESTION)
            .font(theme::icon(12.0))
            .color(colors.text_low);
        child
            .add(egui::Label::new(glyph).selectable(false))
            .on_hover_ui(|ui| {
                ui.set_max_width(inner.width());
                for text in &texts {
                    ui.label(
                        RichText::new(text)
                            .font(theme::small())
                            .color(colors.text_mid),
                    );
                }
            });
    }

    child
        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pressed = action.is_some_and(|(glyph, tooltip)| {
                crate::ui::icon_button(ui, glyph, tooltip).clicked()
            });
            if let Some(hint_text) = hint_text {
                hint(ui, hint_text);
            }
            pressed
        })
        .inner
}

/// Runs `add` with `describe` pointed at this block, and returns what it described.
fn collecting<R>(ui: &mut Ui, id: egui::Id, add: impl FnOnce(&mut Ui) -> R) -> (R, Vec<String>) {
    let key = egui::Id::new(DESCRIBING);
    let gathering = id.with("gathering");
    let outer = ui.data_mut(|data| {
        data.insert_temp(gathering, Vec::<String>::new());
        let outer = data.get_temp::<egui::Id>(key);
        data.insert_temp(key, gathering);
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
        data.remove_temp::<Vec<String>>(gathering)
            .unwrap_or_default()
    });
    (inner, texts)
}

/// What a block is for, or what a setting in it does: kept out of the way, and read from
/// the question mark beside the block's title. Outside a block, a plain `hint`.
pub fn describe(ui: &mut Ui, text: &str) {
    let key = egui::Id::new(DESCRIBING);
    let target = ui.data(|data| data.get_temp::<egui::Id>(key));
    match target {
        Some(id) => ui.data_mut(|data| {
            data.get_temp_mut_or_default::<Vec<String>>(id)
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
    ui.painter().vline(
        rect.left() + 4.0,
        rect.y_range(),
        egui::Stroke::new(1.0, theme::colors().line),
    );
    shown.inner
}

/// A verdict that has to be seen before anything else in its block: a glyph in `tint`,
/// the verdict in plain ink, and a line saying what to do about it.
pub fn notice(ui: &mut Ui, glyph: &str, tint: egui::Color32, title: &str, why: &str) {
    let colors = theme::colors();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(theme::icon(14.0)).color(tint));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(title)
                    .font(theme::label())
                    .color(colors.text_high),
            );
            ui.label(
                RichText::new(why)
                    .font(theme::small())
                    .color(colors.text_mid),
            );
        });
    });
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
    let left = rect.left_center() + vec2(4.0, 0.0);
    let painter = ui.painter();
    painter.text(
        left,
        Align2::LEFT_CENTER,
        glyph,
        theme::icon(11.0),
        colors.text_mid,
    );
    let title_at = left + vec2(17.0, 0.0);
    painter.text(
        title_at,
        Align2::LEFT_CENTER,
        title,
        theme::section(),
        colors.text_high,
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
                .font(theme::figures(11.0))
                .color(theme::colors().text_low),
        );
    }
}

/// Measured values, one to a row with a leader between the name and the value: the
/// figures of a block are rows, never tiles.
pub fn stats(ui: &mut Ui, entries: &[(&str, String)]) {
    readings(ui, entries);
}

/// Points between the name and the value of a reading, which is the least that still
/// reads as two columns.
const READING_GAP: f32 = 10.0;

/// Height of a reading's row, and the room kept between the leader and the text.
const READING_H: f32 = 24.0;
const LEADER_GAP: f32 = 6.0;

/// Lines a value may wrap to before it is cut: a panel size states pixels and
/// millimetres, which is two lines in a column this narrow.
const VALUE_ROWS: usize = 2;

/// Rows of one reading each, the name at the left, the value at the right and a dotted
/// leader between them, for a summary read top to bottom rather than compared across.
///
/// A value too long for the row wraps and then is cut, so that the name beside it stays
/// readable: nothing here is drawn over anything else.
pub fn readings(ui: &mut Ui, entries: &[(&str, String)]) {
    let colors = theme::colors();
    for (key, value) in entries {
        let name =
            ui.painter()
                .layout_no_wrap((*key).to_owned(), theme::reading(), colors.text_low);
        let room = ui.available_width() - name.size().x - READING_GAP;
        let mut job = egui::text::LayoutJob::simple(
            value.clone(),
            theme::reading(),
            colors.text_high,
            room.max(READING_GAP),
        );
        job.wrap.max_rows = VALUE_ROWS;
        job.halign = egui::Align::RIGHT;
        let reading = ui.painter().layout_job(job);

        let height = READING_H.max(reading.size().y + 4.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let painter = ui.painter();
        let name_at = rect.left_center() - vec2(0.0, name.size().y / 2.0);
        let baseline = name_at.y + name.size().y - 3.0;
        let from = name_at.x + name.size().x + LEADER_GAP;
        let to = rect.right() - reading.size().x - LEADER_GAP;
        if reading.rows.len() == 1 && to > from {
            painter.extend(egui::Shape::dotted_line(
                &[egui::pos2(from, baseline), egui::pos2(to, baseline)],
                colors.hairline.gamma_multiply(1.6),
                3.0,
                0.6,
            ));
        }
        painter.galley(name_at, name, colors.text_low);
        painter.galley(
            rect.right_center() - vec2(0.0, reading.size().y / 2.0),
            reading,
            colors.text_high,
        );
    }
}

/// The bar of a running job: filled to `fraction` where it is known, and running on its
/// own where it is not, which is a job that cannot honestly count its work.
pub fn progress_bar(ui: &mut Ui, fraction: Option<f32>) {
    let bar = match fraction {
        Some(fraction) => egui::ProgressBar::new(fraction),
        None => egui::ProgressBar::new(0.0).animate(true),
    };
    ui.add(
        bar.desired_height(theme::BAR_H)
            .corner_radius(theme::R_CONTROL),
    );
}

/// A one pixel divider across the full width of whatever is drawing it.
pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}
