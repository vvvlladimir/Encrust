use egui::{
    Align, Align2, Id, Layout, Response, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, vec2,
};

use crate::i18n::tr;
use crate::ui::theme;
use crate::ui::widgets::{Inline, focus_ring, inline_button};

/// Something found that is not yet the user's: a dashed row of a glyph, its name (`text_low`
/// unless `matches`), a figure such as an address, and an Add button. Answers whether Add
/// was pressed.
pub fn found_row(ui: &mut Ui, glyph: &str, name: &str, figure: &str, matches: bool) -> Response {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::BUTTON_H + 8.0),
        Sense::hover(),
    );
    found_words(ui.painter(), rect, (glyph, name), matches);
    let mut end = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink(4.0))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let add = inline_button(
        &mut end,
        Inline::new(&tr!("found-add")).glyph(crate::ui::icon::ADD),
    );
    ui.painter().text(
        add.rect.left_center() - vec2(10.0, 0.0),
        Align2::RIGHT_CENTER,
        figure,
        theme::figures(12.0),
        colors.text_mid,
    );
    add
}

/// A found row's dashed outline, its glyph and its name, `text_low` unless it `matches`.
fn found_words(
    painter: &egui::Painter,
    rect: egui::Rect,
    (glyph, name): (&str, &str),
    matches: bool,
) {
    let colors = theme::colors();
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let stroke = egui::Stroke::new(1.0, colors.line);
    painter.extend(egui::Shape::dashed_line(
        &corners, stroke, FOUND_DASH, FOUND_DASH,
    ));
    let ink = if matches {
        colors.text_high
    } else {
        colors.text_low
    };
    let left = rect.left_center() + vec2(12.0, 0.0);
    painter.text(
        left,
        Align2::LEFT_CENTER,
        glyph,
        theme::icon(theme::glyph::ROW),
        colors.text_mid,
    );
    painter.text(
        left + vec2(20.0, 0.0),
        Align2::LEFT_CENTER,
        name,
        theme::label(),
        ink,
    );
}

/// A quiet glyph at the end of a row, `right_center` its right end, that says more on a
/// click: a model repair only had to tidy.
pub fn row_mark(
    ui: &mut Ui,
    right_center: egui::Pos2,
    id: Id,
    glyph: &str,
    label: &str,
) -> Response {
    let colors = theme::colors();
    let spot = egui::Rect::from_center_size(
        right_center - vec2(MARK_SIDE / 2.0, 0.0),
        vec2(MARK_SIDE, MARK_SIDE),
    );
    let response = ui.interact(spot, id, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let ink = if response.hovered() {
        colors.text_high
    } else {
        colors.text_low
    };
    ui.painter().text(
        spot.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(theme::glyph::ROW),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// A row mark's square, points.
const MARK_SIDE: f32 = 18.0;

/// The dashes round a found row, points.
const FOUND_DASH: f32 = 4.0;
