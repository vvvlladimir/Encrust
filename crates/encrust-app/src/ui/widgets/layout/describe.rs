use egui::{RichText, Sense, Ui, vec2};

use crate::ui::theme;

/// Where `describe` sends its text while a section's body is being drawn.
const DESCRIBING: &str = "encrust-describing";

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
        None => super::text::hint(ui, text),
    }
}

/// The ring the question mark stands in, the mark, the round target around it a pointer hits, and the
/// most its text runs across before wrapping, points.
const HELP_RING_D: f32 = 16.0;
const HELP_GLYPH: f32 = 10.0;
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
        colors.text_low
    };
    let painter = ui.painter();
    if open || response.hovered() {
        painter.circle_filled(rect.center(), HELP_D / 2.0, colors.hover);
    }
    painter.circle_stroke(
        rect.center(),
        HELP_RING_D / 2.0 - 0.5,
        egui::Stroke::new(1.0, colors.line),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        crate::ui::icon::HELP,
        theme::icon(HELP_GLYPH),
        ink,
    );
    crate::ui::widgets::focus_ring(ui, &response, egui::CornerRadius::same(255));
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
