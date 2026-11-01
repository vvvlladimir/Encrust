//! The frame the Settings and Machine and resin windows share: title row, list at the left,
//! header, page and action bar. See `docs/decisions/0233`.

use egui::{Align, Color32, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::ui::theme::motion;
use crate::ui::{dialog, dialog_head, focus_ring, hover_mix, theme};

/// The most of the screen a window takes, where the screen has it.
const MOST: egui::Vec2 = vec2(1080.0, 720.0);
/// The page's name over the right side, and the action bar under it.
const HEADER_H: f32 = 64.0;
/// The action bar, and the foot of the list beside it at the same height so the two
/// hairlines over them meet.
const FOOTER_H: f32 = theme::DIALOG_BAR_H;
pub(in crate::panels) const LIST_FOOT_H: f32 = FOOTER_H;
/// Room either side of what the right side shows.
pub(in crate::panels) const SIDE_PAD: f32 = 20.0;

/// Shows a window over the plate under `title`, and hands `add` the list's rect down the
/// left and the side's beside it. Answers whether the window was asked to close.
pub(in crate::panels) fn show(
    ctx: &egui::Context,
    id: &str,
    glyph: &str,
    title: &str,
    add: impl FnOnce(&mut Ui, Rect, Rect),
) -> bool {
    let screen = ctx.content_rect().size() - egui::Vec2::splat(2.0 * theme::DIALOG_MARGIN);
    let size = screen.min(MOST).max(egui::Vec2::ZERO);
    let mut close = false;
    let modal = dialog(ctx, egui::Id::new(id), 0).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let head = rect.with_max_y(rect.top() + theme::DIALOG_HEAD_H);
        let list_w = theme::DIALOG_LIST_W.min(rect.width() * theme::DIALOG_LIST_SHARE);
        let list = Rect::from_min_max(
            pos2(rect.left(), head.bottom()),
            pos2(rect.left() + list_w, rect.bottom()),
        );
        let side = Rect::from_min_max(pos2(list.right() + 1.0, head.bottom()), rect.max);
        let list_corner = egui::CornerRadius {
            sw: theme::R_PANEL.sw,
            ..egui::CornerRadius::ZERO
        };
        ui.painter()
            .rect_filled(list, list_corner, theme::colors().base);
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
pub(in crate::panels) fn child(ui: &mut Ui, rect: Rect) -> Ui {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
}

pub(in crate::panels) fn stroke() -> egui::Stroke {
    egui::Stroke::new(1.0, theme::colors().hairline)
}

/// The side split into its header and the rest under it.
pub(in crate::panels) fn with_header(ui: &mut Ui, side: Rect) -> (Ui, Rect) {
    let header = side.with_max_y(side.top() + HEADER_H);
    ui.painter()
        .hline(side.x_range(), header.bottom(), stroke());
    let head = child(ui, header.shrink2(vec2(SIDE_PAD, 0.0)));
    let content = Rect::from_min_max(pos2(side.left(), header.bottom() + 1.0), side.max);
    (head, content)
}

/// `content` split into what scrolls and the action bar along its foot.
pub(in crate::panels) fn with_footer(ui: &mut Ui, content: Rect) -> (Ui, Ui) {
    let split = content.bottom() - FOOTER_H;
    let bar = Rect::from_min_max(pos2(content.left(), split + 1.0), content.max);
    let bar_corner = egui::CornerRadius {
        se: theme::R_PANEL.se,
        ..egui::CornerRadius::ZERO
    };
    ui.painter()
        .rect_filled(bar, bar_corner, theme::colors().base);
    ui.painter().hline(content.x_range(), split, stroke());
    let mut bar = ui.new_child(
        UiBuilder::new()
            .max_rect(bar.shrink2(vec2(SIDE_PAD, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    bar.spacing_mut().item_spacing.x = 4.0;
    (child(ui, content.with_max_y(split)), bar)
}

/// A form, scrolled, with the window's own room either side of it.
pub(in crate::panels) fn scrolled(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
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
pub(in crate::panels) fn tab_button(ui: &mut Ui, label: &str, open: bool) -> egui::Response {
    let colors = theme::colors();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::label(), colors.text_high);
    let size = vec2(galley.size().x, theme::BUTTON_H + 4.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, label));
    let text = match open {
        true => colors.text_high,
        false => hover_mix(ui, &response, colors.text_low, colors.text_high),
    };
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, text);
    // The underline grows out from the middle of the tab coming open.
    let how_open = motion::toggle(ui.ctx(), response.id.with("open"), open, motion::FAST);
    if how_open > 0.0 {
        let half = rect.width() * how_open / 2.0;
        let x = rect.center().x;
        let line = Rect::from_min_max(
            pos2(x - half, rect.bottom() - 2.0),
            pos2(x + half, rect.bottom()),
        );
        ui.painter().rect_filled(line, 0.0, colors.accent);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// The mark down the left edge of the picked row, points.
const PICKED_MARK_W: f32 = 2.0;
/// How much of the name column the box a name is typed in takes.
const NAME_BOX_SHARE: f32 = 0.6;

/// The columns of a table: names, each start as a share of the width, how many lead with
/// words before figures, and the room before the first, where a row may draw a mark.
pub(in crate::panels) struct Table {
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

    /// Where the name of a row in `rect` is typed in: from the name's start over most of its
    /// column.
    pub fn name_box(&self, rect: Rect) -> Rect {
        let column = rect.width() * self.columns.get(1).copied().unwrap_or(1.0) - self.lead;
        let left = rect.left() + self.lead;
        Rect::from_center_size(
            pos2(left + column * NAME_BOX_SHARE / 2.0, rect.center().y),
            vec2(column * NAME_BOX_SHARE, theme::FIELD_H),
        )
    }

    /// One row named `name`, washed and marked at its left edge while `picked`, with `cells`
    /// drawn in their columns: the name in full, the other words dimmer, the figures in the
    /// figure face, dimmer too but on the picked row.
    pub fn row(&self, ui: &mut Ui, name: &str, picked: bool, cells: &[String]) -> egui::Response {
        let colors = theme::colors();
        let size = vec2(ui.available_width(), theme::TABLE_ROW_H);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, picked, name)
        });
        let fill = match picked {
            true => colors.picked_wash,
            false => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, theme::R_CONTROL, fill);
        if picked {
            let mark = rect.with_max_x(rect.left() + PICKED_MARK_W);
            painter.rect_filled(mark, 0.0, colors.accent);
        }
        let figures = match picked {
            true => colors.text_high,
            false => colors.text_mid,
        };
        for (index, text) in cells.iter().enumerate() {
            let (at, align) = self.cell(rect, index);
            let (font, tint) = match index {
                0 => (theme::label(), colors.text_high),
                _ if index < self.words => (theme::label(), colors.text_mid),
                _ => (theme::figures(12.5), figures),
            };
            painter.text(at, align, text, font, tint);
        }
        painter.hline(rect.x_range(), rect.bottom() - 0.5, stroke());
        focus_ring(ui, &response, theme::R_CONTROL);
        response
    }
}
