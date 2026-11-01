use egui::{
    Align, Align2, Color32, Frame, Id, Layout, Rect, RichText, Sense, Ui, UiBuilder, WidgetInfo,
    WidgetType, vec2,
};

use super::{focus_ring, hover_mix};
use crate::i18n::tr;
use crate::ui::theme;

mod found;
mod issue;
mod tag;

pub use found::{found_row, row_mark};
pub use issue::{issue_list, issue_row};
pub use tag::{bullet, tag, tag_label};

/// What a click on a row of a list asked for.
#[derive(Default)]
pub struct RowAction {
    /// The row itself was clicked, which is how a list of one active entry is chosen.
    pub picked: bool,
    /// The trailing icon button was clicked.
    pub removed: bool,
    /// The name typed over the label, once Enter or a click elsewhere takes it.
    pub renamed: Option<String>,
    /// The eye was clicked: what the row stands for is to be shown, or hidden.
    pub toggled: bool,
    /// The row itself, for a caller that hangs a menu on its secondary click.
    pub row: Option<egui::Response>,
}

/// A list the panel adds rows to: support groups, exposure bands, drain holes.
///
/// The gaps between the rows are the container showing through, so one hairline separates
/// them however many there are.
pub fn list<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(theme::colors().hairline)
        .corner_radius(theme::R_CONTROL)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            add(ui)
        })
        .inner
}

/// One row of that list: what it is, the colour it is drawn in, a figure at its right, and
/// the button that drops it. Given a `rename` id, a double click turns the label into a
/// field, as a plate tab does. Given `shown`, an eye beside the figure says whether what
/// the row stands for is drawn, and asks for the other.
///
/// The row takes its click before its contents are laid out, so the button over it is the
/// later widget and wins the press it sits under.
#[allow(clippy::too_many_arguments)]
pub fn list_row(
    ui: &mut Ui,
    label: &str,
    tint: Option<Color32>,
    selected: bool,
    remove: Option<(&str, &str)>,
    rename: Option<Id>,
    aside: Option<&str>,
    shown: Option<bool>,
) -> RowAction {
    let colors = theme::colors();
    let (rect, row) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    let text = colors.text_high;
    let fill = match selected {
        true => colors.picked_wash,
        false => hover_mix(ui, &row, colors.raised, colors.hover),
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    row.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, label));
    focus_ring(ui, &row, 0.0);

    let mut action = RowAction::default();
    let inner = rect.shrink2(vec2(9.0, 0.0));
    let layout = Layout::left_to_right(Align::Center);
    // A child rather than a scope, so the row alone takes its place in the list.
    ui.new_child(UiBuilder::new().max_rect(inner).layout(layout))
        .scope(|ui| {
            if let Some(tint) = tint {
                let (chip, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(chip, theme::R_MARK, tint);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some((glyph, tooltip)) = remove {
                    action.removed = super::icon_button(ui, glyph, tooltip).clicked();
                }
                if let Some(shown) = shown {
                    let (glyph, tooltip) = if shown {
                        (crate::ui::icon::VISIBLE, tr!("list-hide"))
                    } else {
                        (crate::ui::icon::HIDDEN, tr!("list-show"))
                    };
                    action.toggled = super::icon_button(ui, glyph, &tooltip).clicked();
                }
                if let Some(aside) = aside {
                    let figure = RichText::new(aside).font(theme::figures(11.5));
                    ui.label(figure.color(colors.text_low));
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    match rename.filter(|id| is_renaming(ui, *id)) {
                        Some(id) => action.renamed = rename_field(ui, id, |edit| edit),
                        None => {
                            ui.label(RichText::new(label).font(theme::label()).color(text));
                        }
                    }
                });
            });
        });

    if let Some(id) = rename
        && row.double_clicked()
    {
        start_rename(ui, id, label);
    }
    action.picked = !action.removed && !action.toggled && row.clicked();
    action.row = Some(row);
    action
}

/// Turns the name under `id` into a field holding `name`, focused, for [`rename_box`].
pub fn start_rename(ui: &Ui, id: Id, name: &str) {
    ui.data_mut(|data| data.insert_temp(id, name.to_owned()));
    ui.memory_mut(|memory| memory.request_focus(id));
}

/// Whether the name under `id` is being typed.
pub fn is_renaming(ui: &Ui, id: Id) -> bool {
    ui.data(|data| data.get_temp::<String>(id).is_some())
}

/// A table row's name being typed, boxed in `rect`: the new name once Enter or a click
/// elsewhere takes it.
pub fn rename_box(ui: &mut Ui, rect: Rect, id: Id) -> Option<String> {
    let layout = Layout::left_to_right(Align::Center);
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    rename_field(&mut child, id, |edit| {
        super::text_box(edit).background_color(theme::colors().sunken)
    })
}

/// The field a name is typed in. The buffer lives in memory so a keystroke renames
/// nothing; an empty name is refused, and a retype is the only undo it needs.
fn rename_field(
    ui: &mut Ui,
    id: Id,
    style: impl FnOnce(egui::TextEdit<'_>) -> egui::TextEdit<'_>,
) -> Option<String> {
    let mut text = ui.data(|data| data.get_temp::<String>(id).unwrap_or_default());
    let edit = egui::TextEdit::singleline(&mut text)
        .id(id)
        .font(theme::label())
        .desired_width(ui.available_width());
    let response = ui.add(style(edit));
    if !response.lost_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// A name over the line that says more about it, drawn from `left_center`: a machine over
/// its maker and the file it is sliced into.
pub fn two_lines(painter: &egui::Painter, left_center: egui::Pos2, title: &str, detail: &str) {
    let colors = theme::colors();
    let (title_at, detail_at) = (left_center - vec2(0.0, 8.0), left_center + vec2(0.0, 9.0));
    painter.text(
        title_at,
        Align2::LEFT_CENTER,
        title,
        theme::label(),
        colors.text_high,
    );
    painter.text(
        detail_at,
        Align2::LEFT_CENTER,
        detail,
        theme::small(),
        colors.text_low,
    );
}
