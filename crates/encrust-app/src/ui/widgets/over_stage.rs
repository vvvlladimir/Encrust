//! What floats over the stage besides the cards: how to work the viewport, and the notices
//! stacked over its foot. See `docs/design/ui-screens.md`, Over the stage.

use egui::{Align, Layout, Margin, Response, RichText, Sense, Ui, WidgetInfo, WidgetType, vec2};

use super::button::{Skin, filled_button};
use super::{card, icon_button, progress_bar, quiet_button, small_icon_button};
use crate::i18n::tr;
use crate::ui::{icon, theme, theme::motion};

/// How to work the viewport with the tool in hand: a pill of pairs, each a key in
/// `text_high` then what it does in `text_mid`, the pairs split by a short rule.
pub fn hint_bar(ui: &mut Ui, pairs: &[(&str, &str)]) -> Response {
    let colors = theme::colors();
    let painter = ui.painter();
    let galleys: Vec<_> = pairs
        .iter()
        .map(|(key, what)| {
            let key =
                painter.layout_no_wrap((*key).to_owned(), theme::hint_key(), colors.text_high);
            let what = painter.layout_no_wrap((*what).to_owned(), theme::hint(), colors.text_mid);
            (key, what)
        })
        .collect();
    let pairs_w: f32 = galleys
        .iter()
        .map(|(key, what)| key.size().x + HINT_KEY_GAP + what.size().x)
        .sum();
    let rules = galleys.len().saturating_sub(1) as f32;
    let width = pairs_w + rules * (2.0 * HINT_GAP + 1.0) + 2.0 * HINT_PAD;
    let (rect, response) = ui.allocate_exact_size(vec2(width, HINT_H), Sense::hover());
    let read: Vec<String> = pairs
        .iter()
        .map(|(key, what)| format!("{key} {what}"))
        .collect();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, read.join(", ")));

    let painter = ui.painter();
    let pill = egui::CornerRadius::same((HINT_H / 2.0) as u8);
    let stroke = egui::Stroke::new(1.0, colors.hairline);
    let fill = colors.base.gamma_multiply(HINT_OPACITY);
    painter.rect(rect, pill, fill, stroke, egui::StrokeKind::Inside);
    let mut left = rect.left() + HINT_PAD;
    for (at, (key, what)) in galleys.into_iter().enumerate() {
        if at > 0 {
            left += HINT_GAP;
            let rule = egui::Rangef::new(rect.center().y - 6.0, rect.center().y + 6.0);
            painter.vline(left, rule, egui::Stroke::new(1.0, colors.line));
            left += 1.0 + HINT_GAP;
        }
        let top = rect.center().y - key.size().y / 2.0;
        let key_w = key.size().x;
        painter.galley(egui::pos2(left, top), key, colors.text_high);
        left += key_w + HINT_KEY_GAP;
        let what_w = what.size().x;
        painter.galley(egui::pos2(left, top), what, colors.text_mid);
        left += what_w;
    }
    response
}

/// The hint bar's height, the room at its ends, between pairs and inside one, points, and
/// how much of the window's colour it keeps.
const HINT_H: f32 = 28.0;
const HINT_PAD: f32 = 14.0;
const HINT_GAP: f32 = 10.0;
const HINT_KEY_GAP: f32 = 5.0;
const HINT_OPACITY: f32 = 0.88;

/// What was pressed on a [`notice_card`].
#[derive(Default)]
pub struct NoticeReply {
    /// Its one action.
    pub acted: bool,
    /// Its close square.
    pub closed: bool,
}

/// A notice over the stage's foot, in a card of its own: the glyph in `tint` and nothing
/// else coloured, the words with any `detail` under them, at most one `action` as a small button washed in the accent,
/// and, given `close` saying what closing it does, a close square.
pub fn notice_card(
    ui: &mut Ui,
    glyph: &str,
    tint: egui::Color32,
    text: &str,
    detail: Option<&str>,
    action: Option<&str>,
    close: Option<&str>,
) -> NoticeReply {
    let mut reply = NoticeReply::default();
    notice_frame(ui, NOTICE_MIN_W, |ui| {
        ui.label(RichText::new(glyph).font(theme::icon(16.0)).color(tint));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(close) = close {
                reply.closed = icon_button(ui, icon::CANCEL, close).clicked();
            }
            if let Some(action) = action {
                reply.acted = washed_button(ui, action).clicked();
            }
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                words(ui, text);
                if let Some(detail) = detail {
                    let detail = RichText::new(detail)
                        .font(theme::small())
                        .color(theme::colors().text_low);
                    ui.add(egui::Label::new(detail).truncate());
                }
            });
        });
    });
    reply
}

/// A running job over the stage's foot: its glyph, what it is doing, the share done and a
/// quiet Cancel, its bar under them. Answers whether to stop.
pub fn job_notice(ui: &mut Ui, label: &str, fraction: Option<f32>, cancelling: bool) -> bool {
    let colors = theme::colors();
    let mut cancel = false;
    notice_frame(ui, JOB_MIN_W, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = theme::NOTE_GAP;
            ui.horizontal(|ui| {
                spinner(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cancel_text = tr!("common-cancel");
                    cancel = quiet_button(ui, "", &cancel_text, false, !cancelling).clicked();
                    if let Some(fraction) = fraction {
                        let share = format!("{:.0} %", fraction * 100.0);
                        let share = RichText::new(share).font(theme::figures(11.5));
                        ui.label(share.color(colors.text_mid));
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| words(ui, label));
                });
            });
            // A job that cannot count its work runs its bar rather than lie.
            progress_bar(ui, fraction);
        });
    });
    cancel
}

/// A running job's glyph, turning unless motion is reduced.
pub fn spinner(ui: &mut Ui) {
    let ink = theme::colors().accent_soft;
    let (rect, _) = ui.allocate_exact_size(vec2(SPINNER_PT, SPINNER_PT), Sense::hover());
    let galley = ui
        .painter()
        .layout_no_wrap(icon::JOB.to_owned(), theme::icon(SPINNER_PT), ink);
    let turn = motion::spin(ui.ctx()).unwrap_or(0.0) * std::f32::consts::TAU;
    let corner = rect.center() - galley.size() / 2.0;
    let shape = egui::epaint::TextShape::new(corner, galley, ink)
        .with_angle_and_anchor(turn, egui::Align2::CENTER_CENTER);
    ui.painter().add(shape);
}

/// A spinner's glyph, points.
const SPINNER_PT: f32 = 16.0;

fn notice_frame(ui: &mut Ui, min_w: f32, add: impl FnOnce(&mut Ui)) {
    card().inner_margin(NOTICE_MARGIN).show(ui, |ui| {
        // The card's margin and stroke stand outside what is set here.
        let inner_max = NOTICE_MAX_W - NOTICE_MARGIN.sum().x - 2.0;
        let width = ui.available_width().clamp(min_w.min(inner_max), inner_max);
        ui.set_width(width);
        ui.set_min_height(NOTICE_H - NOTICE_MARGIN.sum().y);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = NOTICE_GAP;
            add(ui);
        });
    });
}

fn words(ui: &mut Ui, text: &str) {
    let words = RichText::new(text)
        .font(theme::label())
        .color(theme::colors().text_high);
    ui.add(egui::Label::new(words).truncate());
}

/// The small button a notice offers its action in, washed in the accent.
fn washed_button(ui: &mut Ui, text: &str) -> Response {
    let colors = theme::colors();
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::label(), colors.accent_soft)
        .size()
        .x;
    let skin = Skin {
        fill: colors.accent_wash,
        foreground: colors.accent_soft,
        border: None,
        hovered: colors.accent_wash,
        pressed: colors.active,
    };
    let size = vec2(text_w + 20.0, theme::ICON_SIZE);
    filled_button(ui, "", text, size, &skin, true)
}

/// A notice's height, the most it is wide, and the least a plain one and a job's are.
const NOTICE_H: f32 = 40.0;
const NOTICE_MAX_W: f32 = 440.0;
const NOTICE_MIN_W: f32 = 200.0;
const JOB_MIN_W: f32 = 340.0;
const NOTICE_GAP: f32 = 10.0;
const NOTICE_MARGIN: Margin = Margin {
    left: 12,
    right: 6,
    top: 6,
    bottom: 6,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    fn run(add: impl FnMut(&mut Ui)) {
        fixtures::frame(&fixtures::context(), egui::RawInput::default(), add);
    }

    #[test]
    fn a_hint_bar_is_as_wide_as_what_it_says() {
        let (mut one, mut two) = (0.0, 0.0);
        run(|ui| one = hint_bar(ui, &[("Click", "a point")]).rect.width());
        run(|ui| {
            two = hint_bar(ui, &[("Click", "a point"), ("Esc", "start again")])
                .rect
                .width()
        });
        assert!(
            two > one + 2.0 * HINT_GAP,
            "a second pair widens the pill: {one} then {two}"
        );
        assert!(one < 300.0, "one short pair stays short, got {one}");
    }

    #[test]
    fn a_notice_is_never_wider_than_its_limit() {
        let mut width = 0.0;
        run(|ui| {
            let long = "A model on the plate will not print as it stands ".repeat(8);
            let shown = ui.scope(|ui| {
                notice_card(
                    ui,
                    icon::WARNING,
                    theme::colors().warn,
                    &long,
                    None,
                    None,
                    Some("Put away"),
                )
            });
            let _ = shown.inner;
            width = shown.response.rect.width();
        });
        assert!(
            width <= NOTICE_MAX_W + 1.0,
            "at most {NOTICE_MAX_W} wide, got {width}"
        );
    }
}

/// What was pressed on a [`zoom_bar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomPress {
    Out,
    In,
    Whole,
}

/// Zoom out, the zoom in percent, zoom in and *show the whole*, in a row; drawn inside the
/// card the caller places. `zoom` is 1 at the whole picture.
pub fn zoom_bar(ui: &mut Ui, zoom: f32) -> Option<ZoomPress> {
    ui.spacing_mut().item_spacing.x = 2.0;
    let mut pressed = None;
    if small_icon_button(ui, icon::ZOOM_OUT, &tr!("zoom-out")).clicked() {
        pressed = Some(ZoomPress::Out);
    }
    let percent = format!("{:.0}%", zoom * 100.0);
    ui.add_space(ZOOM_TEXT_PAD);
    ui.label(
        RichText::new(percent)
            .font(theme::small())
            .color(theme::colors().text_mid),
    );
    ui.add_space(ZOOM_TEXT_PAD);
    if small_icon_button(ui, icon::ZOOM_IN, &tr!("zoom-in")).clicked() {
        pressed = Some(ZoomPress::In);
    }
    if small_icon_button(ui, icon::FIT, &tr!("zoom-whole")).clicked() {
        pressed = Some(ZoomPress::Whole);
    }
    pressed
}

/// Room either side of the zoom's percent, points.
const ZOOM_TEXT_PAD: f32 = 6.0;
