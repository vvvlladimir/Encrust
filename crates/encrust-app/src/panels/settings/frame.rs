//! The frame the Settings window and the Machine and resin window are both drawn in: a
//! title row, a list down the left, and beside it a header, what the page shows and an
//! action bar along its foot. See `docs/decisions/0233`.

use egui::{Align, Color32, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::ui::{dialog_frame, dialog_head, focus_ring, theme};

/// The most of the screen a window takes, where the screen has it.
const MOST: egui::Vec2 = vec2(1080.0, 720.0);
/// The page's name over the right side, and the action bar under it.
const HEADER_H: f32 = 64.0;
/// The action bar, and the foot of the list beside it at the same height so the two
/// hairlines over them meet: one button and the room either side of it.
const FOOTER_H: f32 = theme::BUTTON_H + 24.0;
pub(super) const LIST_FOOT_H: f32 = FOOTER_H;
/// Room either side of what the right side shows.
pub(super) const SIDE_PAD: f32 = 20.0;

/// Shows a window over the plate under `title`, and hands `add` the list's rect down the
/// left and the side's beside it. Answers whether the window was asked to close.
pub(super) fn show(
    ctx: &egui::Context,
    id: &str,
    glyph: &str,
    title: &str,
    add: impl FnOnce(&mut Ui, Rect, Rect),
) -> bool {
    let screen = ctx.content_rect().size() - egui::Vec2::splat(2.0 * theme::DIALOG_MARGIN);
    let size = screen.min(MOST).max(egui::Vec2::ZERO);
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new(id))
        .frame(dialog_frame())
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let head = rect.with_max_y(rect.top() + theme::DIALOG_HEAD_H);
            let list_w = theme::DIALOG_LIST_W.min(rect.width() * theme::DIALOG_LIST_SHARE);
            let list = Rect::from_min_max(
                pos2(rect.left(), head.bottom()),
                pos2(rect.left() + list_w, rect.bottom()),
            );
            let side = Rect::from_min_max(pos2(list.right() + 1.0, head.bottom()), rect.max);
            let mut title_row = ui.new_child(
                UiBuilder::new()
                    .max_rect(head.shrink2(vec2(16.0, 0.0)))
                    .layout(Layout::left_to_right(Align::Center)),
            );
            close = dialog_head(&mut title_row, glyph, title);
            let painter = ui.painter();
            painter.hline(rect.x_range(), head.bottom(), stroke());
            painter.vline(list.right(), list.y_range(), stroke());
            add(ui, list, side);
        });
    close || modal.should_close()
}

/// A child laid out top down inside `rect`, and clipped to it.
pub(super) fn child(ui: &mut Ui, rect: Rect) -> Ui {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
}

pub(super) fn stroke() -> egui::Stroke {
    egui::Stroke::new(1.0, theme::colors().hairline)
}

/// The side split into its header and the rest under it.
pub(super) fn with_header(ui: &mut Ui, side: Rect) -> (Ui, Rect) {
    let header = side.with_max_y(side.top() + HEADER_H);
    ui.painter()
        .hline(side.x_range(), header.bottom(), stroke());
    let head = child(ui, header.shrink2(vec2(SIDE_PAD, 0.0)));
    let content = Rect::from_min_max(pos2(side.left(), header.bottom() + 1.0), side.max);
    (head, content)
}

/// `content` split into what scrolls and the action bar along its foot.
pub(super) fn with_footer(ui: &mut Ui, content: Rect) -> (Ui, Ui) {
    let split = content.bottom() - FOOTER_H;
    ui.painter().hline(content.x_range(), split, stroke());
    let bar = Rect::from_min_max(pos2(content.left(), split + 1.0), content.max);
    let mut bar = ui.new_child(
        UiBuilder::new()
            .max_rect(bar.shrink2(vec2(SIDE_PAD, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    bar.spacing_mut().item_spacing.x = 4.0;
    (child(ui, content.with_max_y(split)), bar)
}

/// A form, scrolled, with the window's own room either side of it.
pub(super) fn scrolled(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(SIDE_PAD);
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - SIDE_PAD).max(0.0));
                    ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                    add(ui);
                    ui.add_space(12.0);
                });
            });
        });
}

/// One tab: its name, with the accent under it while it is the one open.
pub(super) fn tab_button(ui: &mut Ui, label: &str, open: bool) -> egui::Response {
    let colors = theme::colors();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::label(), colors.text_high);
    let size = vec2(galley.size().x, theme::BUTTON_H + 4.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, label));
    let text = match (open, response.hovered()) {
        (true, _) | (false, true) => colors.text_high,
        (false, false) => colors.text_low,
    };
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, text);
    if open {
        let line = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom());
        ui.painter().rect_filled(line, 0.0, colors.accent);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// The columns of a table: their names, where each starts as a share of the width, how
/// many lead with words before the figures, and the room before the first, which a row
/// may draw a mark in.
pub(super) struct Table {
    /// The column names, in the window's language.
    pub heads: fn() -> Vec<String>,
    pub columns: &'static [f32],
    pub words: usize,
    pub lead: f32,
}

impl Table {
    /// The row of column names, over a hairline.
    pub fn head_row(&self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
        let colors = theme::colors();
        for (index, head) in (self.heads)().into_iter().enumerate() {
            let (at, align) = self.cell(rect, index);
            ui.painter()
                .text(at, align, head, theme::small(), colors.text_low);
        }
        ui.painter().hline(rect.x_range(), rect.bottom(), stroke());
    }

    /// Where cell `index` of a row in `rect` is drawn from: words from their left, the
    /// figures after them from their right, flush against the start of the next column.
    pub fn cell(&self, rect: Rect, index: usize) -> (egui::Pos2, egui::Align2) {
        let x = |share: f32| rect.left() + rect.width() * share;
        let y = rect.center().y;
        if index == 0 {
            return (pos2(rect.left() + self.lead, y), egui::Align2::LEFT_CENTER);
        }
        if index < self.words {
            return (pos2(x(self.columns[index]), y), egui::Align2::LEFT_CENTER);
        }
        let right = self
            .columns
            .get(index + 1)
            .map_or(rect.right() - 30.0, |next| x(*next) - 12.0);
        (pos2(right, y), egui::Align2::RIGHT_CENTER)
    }

    /// One row named `name`, washed while `picked`, with `cells` drawn in their columns:
    /// the name in full, the other words dimmer, the figures in the figure face.
    pub fn row(&self, ui: &mut Ui, name: &str, picked: bool, cells: &[String]) -> egui::Response {
        let colors = theme::colors();
        let size = vec2(ui.available_width(), theme::TABLE_ROW_H);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, picked, name)
        });
        let fill = match (picked, response.hovered()) {
            (true, _) => colors.accent_wash,
            (false, true) => colors.hover,
            (false, false) => Color32::TRANSPARENT,
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, theme::R_CONTROL, fill);
        for (index, text) in cells.iter().enumerate() {
            let (at, align) = self.cell(rect, index);
            let (font, tint) = match index {
                0 => (theme::label(), colors.text_high),
                _ if index < self.words => (theme::label(), colors.text_mid),
                _ => (theme::figures(12.5), colors.text_high),
            };
            painter.text(at, align, text, font, tint);
        }
        painter.hline(rect.x_range(), rect.bottom() - 0.5, stroke());
        focus_ring(ui, &response, theme::R_CONTROL);
        response
    }
}
