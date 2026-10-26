use egui::layers::ShapeIdx;
use egui::{Frame, RichText, Sense, Ui, vec2};

use super::section::trailing_hairline;
use crate::i18n::tr;
use crate::ui::theme;

/// Where `describe` sends its text while a section's body is being drawn.
const DESCRIBING: &str = "encrust-describing";

/// How far what a switch reveals is set in under it, points.
const NEST_INDENT: i8 = 14;

/// Draws the body of the block `id` with `describe` pointed at it, keeping what it said
/// for the block's question mark, which is drawn before the body.
pub fn describing<R>(ui: &mut Ui, id: egui::Id, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (inner, texts) = collecting(ui, id, add);
    ui.data_mut(|data| data.insert_temp(id.with("texts"), texts));
    inner
}

/// What the body of the block `id` described when it was last drawn.
pub fn described(ui: &Ui, id: egui::Id) -> Vec<String> {
    ui.data(|data| data.get_temp::<Vec<String>>(id.with("texts")))
        .unwrap_or_default()
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

/// The help glyph's size, the round target around it a pointer hits, and the most its text
/// runs across before wrapping, points.
const HELP_GLYPH: f32 = 14.0;
const HELP_D: f32 = 20.0;
const HELP_W: f32 = 280.0;

/// The question mark beside a title that reads out `texts`: on hover, and in a popup a
/// click keeps open until the next click. Faint at rest, so it never pulls the eye.
pub fn help_mark<T: AsRef<str>>(ui: &mut Ui, texts: &[T]) {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(HELP_D, HELP_D), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let joined: Vec<&str> = texts.iter().map(AsRef::as_ref).collect();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, joined.join(" "))
    });
    let open = egui::Popup::menu(&response).is_open();
    let ink = if open {
        colors.accent_soft
    } else if response.hovered() {
        colors.text_high
    } else {
        colors.text_low.gamma_multiply(0.75)
    };
    let painter = ui.painter();
    if open || response.hovered() {
        painter.circle_filled(rect.center(), HELP_D / 2.0, colors.hover);
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        crate::ui::icon::HELP,
        theme::icon(HELP_GLYPH),
        ink,
    );
    super::focus_ring(ui, &response, egui::CornerRadius::same(255));
    let show = |ui: &mut Ui| {
        ui.set_max_width(HELP_W);
        for text in &joined {
            ui.label(
                RichText::new(*text)
                    .font(theme::small())
                    .color(colors.text_mid),
            );
        }
    };
    let response = if open {
        response
    } else {
        response.on_hover_ui(show)
    };
    egui::Popup::menu(&response).show(show);
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

/// A body that scrolls once it outgrows the card, and a foot under it that stays in
/// view. `max_h` is the card's whole height, points, what is already drawn in it included.
///
/// The foot is measured as it is drawn, so the body is held to last frame's measure, and
/// a frame whose foot changed height is drawn again.
pub fn body_and_foot<T>(
    ui: &mut Ui,
    max_h: f32,
    state: &mut T,
    body: impl FnOnce(&mut Ui, &mut T),
    foot: impl FnOnce(&mut Ui, &mut T),
) {
    let id = ui.id().with("foot-h");
    let foot_h = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0);
    let room = (max_h - ui.min_rect().height() - foot_h).max(0.0);
    ui.data_mut(|data| data.remove::<(ShapeIdx, f32)>(trailing_hairline()));
    egui::ScrollArea::vertical()
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| body(ui, state));
    let body_bottom = ui.min_rect().bottom();
    let drawn = ui.scope(|ui| foot(ui, state)).response.rect.height();
    // With no foot the last block's hairline would stand square over the card's rounded
    // edge, so the card ends on that block instead.
    let trailing = ui.data(|data| data.get_temp::<(ShapeIdx, f32)>(trailing_hairline()));
    if let Some((shape, bottom)) = trailing
        && drawn <= 0.0
        && (bottom - body_bottom).abs() < 0.5
    {
        ui.painter().set(shape, egui::Shape::Noop);
    }
    if (drawn - foot_h).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, drawn));
        ui.ctx()
            .request_discard("the foot of a card changed height");
    }
}

/// The frame of anything that floats over the viewport.
pub fn card() -> Frame {
    Frame::new()
        .fill(theme::colors().panel)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_SURFACE)
        .shadow(theme::shadow())
}

/// The foot of a card down a side of the stage, set apart on the window's colour and
/// rounded to fit inside the card's outline.
pub fn card_foot() -> Frame {
    let r = theme::R_SURFACE.sw.saturating_sub(1);
    Frame::new()
        .fill(theme::colors().base)
        .inner_margin(theme::PANEL_MARGIN)
        .corner_radius(egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: r,
            se: r,
        })
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

/// A `subheading` carrying the switch that turns its group on. Returns whether it was
/// flipped.
pub fn subheading_with_switch(ui: &mut Ui, title: &str, on: &mut bool) -> bool {
    ui.add_space(8.0);
    let flipped = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .font(theme::small())
                    .color(theme::colors().text_low),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                super::bare_switch(ui, on, title).changed()
            })
            .inner
        })
        .inner;
    ui.add_space(2.0);
    flipped
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
pub fn stats(ui: &mut Ui, entries: &[(String, String)]) {
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
pub fn readings(ui: &mut Ui, entries: &[(String, String)]) {
    let colors = theme::colors();
    for (key, value) in entries {
        let name = ui
            .painter()
            .layout_no_wrap(key.clone(), theme::reading(), colors.text_low);
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

/// What a tool is busy with and the bar of its run, and where the run can be stopped a way
/// to stop it. `step` is the step in words and the share done, for a run that counts; the
/// bar runs without a share for one that does not. Answers whether it was asked to stop.
pub fn busy(ui: &mut Ui, label: &str, step: Option<(&str, f32)>, stoppable: bool) -> bool {
    super::tone(ui, label, theme::colors().text_mid);
    progress_bar(ui, step.map(|(_, share)| share));
    if let Some((step, _)) = step {
        super::tone(ui, step, theme::colors().text_low);
    }
    stoppable
        && super::secondary_button(ui, crate::ui::icon::CANCEL, &tr!("common-cancel")).clicked()
}

/// A one pixel divider across the full width of whatever is drawing it.
pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}

/// A one pixel divider standing `height` points tall, between groups laid out in a row.
pub fn vertical_hairline(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}

/// The title row of a window over the plate: its glyph, its name, and the way out.
/// Answers whether the way out was taken.
pub fn dialog_head(ui: &mut Ui, glyph: &str, title: &str) -> bool {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(glyph)
                .font(theme::icon(15.0))
                .color(colors.text_low),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(title)
                .font(theme::dialog_title())
                .color(colors.text_high),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            super::icon_button(ui, crate::ui::icon::CANCEL, &tr!("dialog-close")).clicked()
        })
        .inner
    })
    .inner
}

/// The frame of a window over the plate: the panel's fill, a hairline edge, the shadow.
pub fn dialog_frame() -> Frame {
    Frame::new()
        .fill(theme::colors().panel)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_PANEL)
        .shadow(theme::shadow())
}
