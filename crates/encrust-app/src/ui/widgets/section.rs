//! The inspector's blocks: a heading that folds the body, with a question mark, a hint
//! and actions or a switch at its right.

use egui::{Frame, Sense, Ui, UiBuilder, vec2};

use super::layout::{described, describing, help_mark, hint};
use crate::ui::{kept, theme};

/// One block of the inspector: a heading that folds it, an optional hint on the right,
/// the body, and a hairline under the lot. What the body `describe`s is a question mark
/// beside the title, read on hover.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    hint_text: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    block(ui, title, hint_text, Trailing::Nothing, add).1
}

/// A block like `section` with one icon button at the right of its heading, for the
/// action that belongs to the whole block. Returns whether it was pressed.
pub fn section_with_action<R>(
    ui: &mut Ui,
    title: &str,
    action: (&str, &str),
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, Option<R>) {
    let (pressed, inner) = block(ui, title, None, Trailing::Actions(&[action]), add);
    (pressed.is_some(), inner)
}

/// A block like `section` with a few icon buttons at the right of its heading, left to
/// right as listed, for the actions over the whole block. Returns which was pressed.
pub fn section_with_actions<R>(
    ui: &mut Ui,
    title: &str,
    actions: &[(&str, &str)],
    add: impl FnOnce(&mut Ui) -> R,
) -> (Option<usize>, Option<R>) {
    block(ui, title, None, Trailing::Actions(actions), add)
}

/// A block whose heading carries the switch that turns the whole of it on, so the switch
/// is not named a second time under it. Returns whether it was flipped.
pub fn section_with_switch<R>(
    ui: &mut Ui,
    title: &str,
    on: &mut bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> (bool, Option<R>) {
    let (flipped, inner) = block(ui, title, None, Trailing::Switch(on), add);
    (flipped.is_some(), inner)
}

/// What stands at the right end of a block's heading.
enum Trailing<'a> {
    Nothing,
    /// Icon buttons, each its glyph and its tooltip, left to right.
    Actions(&'a [(&'a str, &'a str)]),
    Switch(&'a mut bool),
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
    trailing: Trailing<'_>,
    add: impl FnOnce(&mut Ui) -> R,
) -> (Option<usize>, Option<R>) {
    let id = ui.id().with(("section", title));
    let open = kept::get::<bool>(ui.ctx(), id.with("open")) != Some(false);
    let pressed = block_heading(ui, id, title, open, hint_text, trailing);
    let inner = open.then(|| {
        Frame::new()
            .inner_margin(BODY_MARGIN)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                describing(ui, id, add)
            })
            .inner
    });
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    let shape = ui.painter().add(egui::Shape::rect_filled(
        rect,
        0.0,
        theme::colors().hairline,
    ));
    ui.data_mut(|data| data.insert_temp(trailing_hairline(), (shape, rect.bottom())));
    (pressed, inner)
}

/// Where the hairline under the last block drawn is kept, so a card ending on it can
/// take it away.
pub(super) fn trailing_hairline() -> egui::Id {
    egui::Id::new("trailing-hairline")
}

/// The heading row of a block: caret and title, which fold it, the question mark that
/// reads out what the body described, and the hint and action at the right.
fn block_heading(
    ui: &mut Ui,
    id: egui::Id,
    title: &str,
    open: bool,
    hint_text: Option<&str>,
    trailing: Trailing<'_>,
) -> Option<usize> {
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
    fold.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, title)
    });
    super::focus_ring(&child, &fold, theme::R_CONTROL);
    if fold
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        kept::set(child.ctx(), id.with("open"), &!open);
    }

    let texts = described(&child, id);
    if !texts.is_empty() {
        help_mark(&mut child, &texts);
    }

    child
        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pressed = trailing_controls(ui, title, trailing);
            if let Some(hint_text) = hint_text {
                hint(ui, hint_text);
            }
            pressed
        })
        .inner
}

/// The buttons or the switch at the right of a heading, laid right to left; which was used.
fn trailing_controls(ui: &mut Ui, title: &str, trailing: Trailing<'_>) -> Option<usize> {
    match trailing {
        Trailing::Nothing => None,
        Trailing::Actions(actions) => {
            ui.spacing_mut().item_spacing.x = theme::ICON_GAP;
            // Laid right to left, so the last listed stands at the end.
            let mut pressed = None;
            for (at, (glyph, tooltip)) in actions.iter().enumerate().rev() {
                if crate::ui::icon_button(ui, glyph, tooltip).clicked() {
                    pressed = Some(at);
                }
            }
            pressed
        }
        Trailing::Switch(on) => {
            // In line with the switches of the body, which sit a full pad in.
            ui.add_space(4.0);
            super::bare_switch(ui, on, title).changed().then_some(0)
        }
    }
}
