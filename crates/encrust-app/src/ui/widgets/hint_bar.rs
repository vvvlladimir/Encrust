//! How to work the viewport with the tool in hand, fitted to the room over the stage.
//! See `docs/design/ui-screens.md`, Over the stage.

use std::sync::Arc;

use egui::{Galley, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::ui::theme;

/// The hint bar's height, the room at its ends, between pairs and inside one, points, and
/// how much of the window's colour it keeps.
const HINT_H: f32 = 28.0;
const HINT_PAD: f32 = 14.0;
const HINT_GAP: f32 = 10.0;
const HINT_KEY_GAP: f32 = 5.0;
const HINT_OPACITY: f32 = 0.88;

/// What the rule between two pairs takes, its gaps included.
const RULE_W: f32 = 2.0 * HINT_GAP + 1.0;

/// How to work the viewport with the tool in hand, on one line at most `room` wide: pairs of
/// a key in `text_high` then what it does in `text_mid`, split by short rules. Short of room,
/// only the `keys` show, each saying what it does under the pointer.
pub fn hint_bar(ui: &mut Ui, pairs: &[(&str, &str)], keys: &[&str], room: f32) -> Response {
    let colors = theme::colors();
    let painter = ui.painter();
    let layout = |text: &str, font, color| painter.layout_no_wrap(text.to_owned(), font, color);
    let mut items: Vec<Vec<Arc<Galley>>> = pairs
        .iter()
        .map(|(key, what)| {
            vec![
                layout(key, theme::hint_key(), colors.text_high),
                layout(what, theme::hint(), colors.text_mid),
            ]
        })
        .collect();
    let named = width(&items) <= room;
    if !named {
        items = keys
            .iter()
            .map(|key| vec![layout(key, theme::hint_key(), colors.text_high)])
            .collect();
    }
    let (rect, response) = ui.allocate_exact_size(vec2(width(&items), HINT_H), Sense::hover());
    let read: Vec<String> = pairs
        .iter()
        .map(|(key, what)| format!("{key} {what}"))
        .collect();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, read.join(", ")));
    let spans = paint(ui, rect, &items);
    if !named {
        for (at, (span, (_, what))) in spans.into_iter().zip(pairs).enumerate() {
            let id = response.id.with(at);
            ui.interact(span, id, Sense::hover()).on_hover_text(*what);
        }
    }
    response
}

/// The bar's width holding `items`, each a run of galleys.
fn width(items: &[Vec<Arc<Galley>>]) -> f32 {
    let runs: f32 = items.iter().map(|run| run_w(run)).sum();
    runs + items.len().saturating_sub(1) as f32 * RULE_W + 2.0 * HINT_PAD
}

fn run_w(run: &[Arc<Galley>]) -> f32 {
    let galleys: f32 = run.iter().map(|galley| galley.size().x).sum();
    galleys + run.len().saturating_sub(1) as f32 * HINT_KEY_GAP
}

/// Paints the pill and `items` along it; answers where each item stands.
fn paint(ui: &Ui, rect: Rect, items: &[Vec<Arc<Galley>>]) -> Vec<Rect> {
    let colors = theme::colors();
    let painter = ui.painter();
    let pill = egui::CornerRadius::same((HINT_H / 2.0) as u8);
    let stroke = egui::Stroke::new(1.0, colors.hairline);
    let fill = colors.base.gamma_multiply(HINT_OPACITY);
    painter.rect(rect, pill, fill, stroke, egui::StrokeKind::Inside);
    let y = rect.center().y;
    let mut left = rect.left() + HINT_PAD;
    let mut spans = Vec::with_capacity(items.len());
    for (at, run) in items.iter().enumerate() {
        if at > 0 {
            let x = left + HINT_GAP;
            let rule = egui::Rangef::new(y - 6.0, y + 6.0);
            painter.vline(x, rule, egui::Stroke::new(1.0, colors.line));
            left += RULE_W;
        }
        spans.push(Rect::from_x_y_ranges(
            left..=left + run_w(run),
            rect.y_range(),
        ));
        for galley in run {
            let top = y - galley.size().y / 2.0;
            painter.galley(pos2(left, top), galley.clone(), colors.text_high);
            left += galley.size().x + HINT_KEY_GAP;
        }
        left -= HINT_KEY_GAP;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    fn run(add: impl FnMut(&mut Ui)) {
        fixtures::frame(&fixtures::context(), egui::RawInput::default(), add);
    }

    const PAIRS: [(&str, &str); 2] = [("Click", "a point"), ("Esc", "start again")];
    const KEYS: [&str; 2] = ["Click", "Esc"];

    #[test]
    fn a_hint_bar_with_room_is_as_wide_as_what_it_says() {
        let (mut one, mut two) = (0.0, 0.0);
        run(|ui| one = hint_bar(ui, &PAIRS[..1], &KEYS[..1], 1000.0).rect.width());
        run(|ui| two = hint_bar(ui, &PAIRS, &KEYS, 1000.0).rect.width());
        assert!(
            two > one + 2.0 * HINT_GAP,
            "a second pair widens the pill: {one} then {two}"
        );
        assert!(one < 300.0, "one short pair stays short, got {one}");
    }

    #[test]
    fn a_hint_bar_short_of_room_shows_its_keys_alone_on_one_line() {
        let (mut named, mut keys) = (Rect::NOTHING, Rect::NOTHING);
        run(|ui| named = hint_bar(ui, &PAIRS, &KEYS, 1000.0).rect);
        run(|ui| keys = hint_bar(ui, &PAIRS, &KEYS, named.width() - 1.0).rect);
        assert!(
            keys.width() < named.width() - 1.0,
            "fits {}, got {}",
            named.width() - 1.0,
            keys.width()
        );
        assert!(
            (keys.height() - HINT_H).abs() < 1e-4,
            "one line, got {keys:?}"
        );
    }
}
