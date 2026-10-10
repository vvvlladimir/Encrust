use egui::{
    Align, Align2, Color32, Frame, Id, Layout, Response, RichText, Sense, Ui, UiBuilder, vec2,
};

use crate::ui::theme;

/// What a click on a row of a list asked for.
#[derive(Default)]
pub struct RowAction {
    /// The row itself was clicked, which is how a list of one active entry is chosen.
    pub picked: bool,
    /// The trailing icon button was clicked.
    pub removed: bool,
    /// The name typed over the label, once Enter or a click elsewhere takes it.
    pub renamed: Option<String>,
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

/// One row of that list: what it is, the colour it is drawn in, and the button that drops it.
/// Given a `rename` id, a double click turns the label into a field, as a plate tab does.
///
/// The row takes its click before its contents are laid out, so the button over it is the
/// later widget and wins the press it sits under.
pub fn list_row(
    ui: &mut Ui,
    label: &str,
    tint: Option<Color32>,
    selected: bool,
    remove: Option<(&str, &str)>,
    rename: Option<Id>,
) -> RowAction {
    let colors = theme::colors();
    let (rect, row) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    let (fill, text) = match (selected, row.hovered()) {
        (true, _) => (colors.picked_wash, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (colors.raised, colors.text_high),
    };
    ui.painter().rect_filled(rect, 0.0, fill);

    let mut action = RowAction::default();
    let inner = rect.shrink2(vec2(9.0, 0.0));
    let layout = Layout::left_to_right(Align::Center);
    // A child rather than a scope, so the row alone takes its place in the list.
    ui.new_child(UiBuilder::new().max_rect(inner).layout(layout))
        .scope(|ui| {
            if let Some(tint) = tint {
                let (chip, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(chip, 2.0, tint);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some((glyph, tooltip)) = remove {
                    action.removed = super::icon_button(ui, glyph, tooltip).clicked();
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    match rename.filter(|id| is_renaming(ui, *id)) {
                        Some(id) => action.renamed = rename_field(ui, id),
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
        ui.data_mut(|data| data.insert_temp(id, label.to_owned()));
        ui.memory_mut(|memory| memory.request_focus(id));
    }
    action.picked = !action.removed && row.clicked();
    action
}

fn is_renaming(ui: &Ui, id: Id) -> bool {
    ui.data(|data| data.get_temp::<String>(id).is_some())
}

/// The field a name is typed in. The buffer lives in memory so a keystroke renames
/// nothing; an empty name is refused, and a retype is the only undo it needs.
fn rename_field(ui: &mut Ui, id: Id) -> Option<String> {
    let mut text = ui.data(|data| data.get_temp::<String>(id).unwrap_or_default());
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .font(theme::label())
            .desired_width(ui.available_width()),
    );
    if !response.lost_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// One finding in a list of them: a dot in its colour, where it is, and what it is at the
/// right. The whole row is the button that goes there.
pub fn issue_row(
    ui: &mut Ui,
    tint: Color32,
    place: &str,
    detail: &str,
    selected: bool,
) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), theme::ROW_H), Sense::click());
    let fill = match (selected, response.hovered()) {
        (true, _) => colors.picked_wash,
        (false, true) => colors.hover,
        (false, false) => colors.raised,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);

    let dot = rect.left_center() + vec2(14.0, 0.0);
    painter.circle_filled(dot, 3.5, tint);
    let place_color = if selected {
        colors.accent_soft
    } else {
        colors.text_high
    };
    painter.text(
        dot + vec2(11.0, 0.0),
        Align2::LEFT_CENTER,
        place,
        theme::label(),
        place_color,
    );
    painter.text(
        rect.right_center() - vec2(12.0, 0.0),
        Align2::RIGHT_CENTER,
        detail,
        theme::figures(11.0),
        colors.text_mid,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
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
