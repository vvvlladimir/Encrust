//! A choice of how the window looks, drawn as a small picture of it. See
//! `docs/design/ui-design-system.md`, Theme card.

use egui::{Rect, Response, Sense, Stroke, StrokeKind, Theme, Ui, WidgetInfo, WidgetType};
use egui::{pos2, vec2};

use super::{focus_ring, hover_mix};
use crate::ui::theme::{self, Palette};

/// What a card's picture shows: one palette, or both halved for the system's choice.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardLook {
    One(Theme),
    Both,
}

/// A card 140 points wide: the picture, then a radio, the choice's name and a line under it.
pub fn theme_card(ui: &mut Ui, look: CardLook, name: &str, sub: &str, chosen: bool) -> Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, chosen, name));
    let picture = Rect::from_min_size(rect.min, vec2(CARD_W, PICTURE_H));
    match look {
        CardLook::One(theme) => paint_picture(ui.painter(), picture, theme::palette_of(theme)),
        CardLook::Both => {
            let (left, right) = picture.split_left_right_at_fraction(0.5);
            let light = theme::palette_of(Theme::Light);
            paint_picture(&ui.painter().with_clip_rect(left), picture, light);
            let dark = theme::palette_of(Theme::Dark);
            paint_picture(&ui.painter().with_clip_rect(right), picture, dark);
        }
    }
    let painter = ui.painter();
    let foot = Rect::from_min_max(pos2(rect.left(), picture.bottom()), rect.max);
    painter.hline(
        rect.x_range(),
        foot.top(),
        Stroke::new(1.0, colors.hairline),
    );
    let dot = pos2(foot.left() + PAD + RADIO_R, foot.center().y);
    let ring = if chosen { colors.accent } else { colors.line };
    painter.circle_stroke(dot, RADIO_R, Stroke::new(1.5, ring));
    if chosen {
        painter.circle_filled(dot, RADIO_R - 3.5, colors.accent);
    }
    let text_x = dot.x + RADIO_R + 8.0;
    let name_at = pos2(text_x, foot.center().y - 1.0);
    painter.text(
        name_at,
        egui::Align2::LEFT_BOTTOM,
        name,
        theme::label(),
        colors.text_high,
    );
    let sub_at = pos2(text_x, foot.center().y + 1.0);
    painter.text(
        sub_at,
        egui::Align2::LEFT_TOP,
        sub,
        theme::small(),
        colors.text_low,
    );
    let edge = match chosen {
        true => Stroke::new(1.5, colors.accent),
        false => Stroke::new(1.0, hover_mix(ui, &response, colors.hairline, colors.line)),
    };
    painter.rect_stroke(rect, theme::R_SURFACE, edge, StrokeKind::Inside);
    focus_ring(ui, &response, theme::R_SURFACE);
    response
}

/// The window in small: its base, a panel down the left and a card with the accent's bar.
fn paint_picture(painter: &egui::Painter, picture: Rect, palette: &Palette) {
    let top = egui::CornerRadius {
        nw: theme::R_SURFACE.nw,
        ne: theme::R_SURFACE.ne,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(picture, top, palette.base);
    let inner = picture.shrink(INSET);
    let side = Rect::from_min_size(inner.min, vec2(SIDE_W, inner.height()));
    painter.rect_filled(side, MINI_R, palette.panel);
    let card = Rect::from_min_max(
        pos2(side.right() + INSET, inner.top()),
        pos2(inner.right(), inner.top() + MINI_CARD_H),
    );
    painter.rect_filled(card, MINI_R, palette.panel);
    let bar = Rect::from_min_size(
        card.left_bottom() + vec2(INSET, -INSET - BAR_H),
        vec2(card.width() - 2.0 * INSET, BAR_H),
    );
    painter.rect_filled(bar, MINI_R, palette.accent);
}

/// Points: a card's size, its picture's height, the room inside, a radio's radius, and the
/// parts of the picture.
const CARD_W: f32 = 140.0;
const CARD_H: f32 = 124.0;
const PICTURE_H: f32 = 74.0;
const PAD: f32 = 10.0;
const RADIO_R: f32 = 7.0;
const INSET: f32 = 8.0;
const SIDE_W: f32 = 34.0;
const MINI_CARD_H: f32 = 40.0;
const BAR_H: f32 = 5.0;
const MINI_R: f32 = 3.0;
