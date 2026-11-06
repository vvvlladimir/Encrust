//! A condition typed in the code face, with the list of the names it can read beside it.

use egui::{Ui, vec2};

use super::name_for_reader;
use super::text::{field_error, row_name, text_box};
use crate::i18n::tr;
use crate::ui::widgets::{DropdownRow, dropdown_menu_wide, dropdown_row, group_name, icon_button};
use crate::ui::{icon, theme};

/// A name a condition can read, as the list beside its box offers it: the name as typed,
/// what it means, and the heading it is listed under.
pub struct ConditionName {
    pub name: String,
    pub meaning: String,
    pub group: String,
}

/// A condition's row: its name, why it does not read, what the box says while empty, and
/// the names its list offers.
pub struct ConditionRow<'a> {
    pub label: &'a str,
    pub error: Option<&'a str>,
    pub empty: &'a str,
    pub names: &'a [ConditionName],
}

/// A named condition typed in the code face, checked as it is typed: while `error` says why
/// it does not read, the box is ringed in `danger` and the why stands under it on one line,
/// read whole on hover. The button at its right adds a name from the list to the end of
/// what is typed. Returns whether the text changed.
pub fn condition_row(ui: &mut Ui, row: &ConditionRow<'_>, value: &mut String) -> bool {
    let colors = theme::colors();
    let mut changed = false;
    ui.horizontal(|ui| {
        row_name(ui, row.label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let list = icon_button(ui, icon::VARIABLE, &tr!("condition-insert"));
            if let Some(name) = names_menu(&list, row.names) {
                append_name(value, &name);
                changed = true;
            }
            let field = text_box(egui::TextEdit::singleline(value))
                .font(theme::code(12.0))
                .hint_text(row.empty)
                .background_color(colors.sunken);
            let response = ui.add_sized(vec2(ui.available_width(), theme::FIELD_H), field);
            name_for_reader(ui, response.id, row.label);
            if row.error.is_some() {
                ui.painter().rect_stroke(
                    response.rect,
                    theme::R_CONTROL,
                    egui::Stroke::new(1.0, colors.danger),
                    egui::StrokeKind::Inside,
                );
            }
            changed |= response.changed();
        });
    });
    if let Some(error) = row.error {
        ui.horizontal(|ui| {
            ui.add_space(theme::ROW_NAME_W + ui.spacing().item_spacing.x);
            field_error(ui, error);
        });
    }
    changed
}

/// The names under the button, each by what it means with the name at its right, under
/// their headings; answers the one chosen.
fn names_menu(button: &egui::Response, names: &[ConditionName]) -> Option<String> {
    dropdown_menu_wide(button, MENU_W, |ui| {
        let scroll = egui::ScrollArea::vertical().max_height(MENU_MAX_H);
        scroll
            .show(ui, |ui| {
                let mut chosen = None;
                let mut group = None;
                for entry in names {
                    if group != Some(entry.group.as_str()) {
                        group = Some(entry.group.as_str());
                        group_name(ui, &entry.group);
                    }
                    let row = DropdownRow {
                        lead: None,
                        name: &entry.meaning,
                        aside: Some(&entry.name),
                        chosen: false,
                    };
                    if dropdown_row(ui, &row).clicked() {
                        chosen = Some(entry.name.clone());
                    }
                }
                chosen
            })
            .inner
    })
    .flatten()
}

/// The list's width and the most of it shown before it scrolls, points.
const MENU_W: f32 = 340.0;
const MENU_MAX_H: f32 = 360.0;

/// `name` added to the end of `value`, a space before it unless it opens the text, a
/// parenthesis or a negation.
fn append_name(value: &mut String, name: &str) {
    if !(value.is_empty() || value.ends_with([' ', '(', '!'])) {
        value.push(' ');
    }
    value.push_str(name);
}

#[cfg(test)]
mod tests {
    use super::append_name;

    #[test]
    fn a_name_joins_the_condition_after_a_space_unless_it_follows_an_opening() {
        let mut typed = String::new();
        append_name(&mut typed, "height_mm");
        assert_eq!(typed, "height_mm");
        typed.push_str(" >");
        append_name(&mut typed, "big_ml");
        assert_eq!(typed, "height_mm > big_ml");
        let mut call = "max(".to_owned();
        append_name(&mut call, "width_mm");
        assert_eq!(call, "max(width_mm");
        let mut not = "!".to_owned();
        append_name(&mut not, "sound");
        assert_eq!(not, "!sound");
    }
}
