use std::ops::RangeInclusive;

use core_geometry::Scalar;
use egui::{Align, Color32, Layout, Ui};

use super::{Field, boxed_field, field_label};
use crate::i18n::tr;
use crate::ui::widgets::icon_button;
use crate::ui::{icon, theme};

/// A named number: the label and its unit on the left, a box of one width on the right.
pub fn number_row(
    ui: &mut Ui,
    label: &str,
    value: &mut Scalar,
    unit: &str,
    speed: f64,
    range: RangeInclusive<Scalar>,
    decimals: usize,
) -> bool {
    let field = Field {
        name: label,
        unit,
        width: theme::FIELD_W,
        flagged: false,
    };
    row(ui, label, |ui| {
        boxed_field(ui, &field, value, speed, range, Some(decimals))
    })
}

/// What was done to a number the window had changed on the user's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    Untouched,
    Edited,
    /// The button that puts back what it was before the change.
    Reverted,
}

/// A named number the window changed on the user's behalf: while `was` is `Some`, its label
/// is in the warning colour and a button beside it puts that value back.
#[allow(clippy::too_many_arguments)]
pub fn carried_row(
    ui: &mut Ui,
    label: &str,
    value: &mut Scalar,
    unit: &str,
    speed: f64,
    range: RangeInclusive<Scalar>,
    decimals: usize,
    was: Option<Scalar>,
) -> Carried {
    let colors = theme::colors();
    let Some(was) = was else {
        return if number_row(ui, label, value, unit, speed, range, decimals) {
            Carried::Edited
        } else {
            Carried::Untouched
        };
    };
    let mut carried = Carried::Untouched;
    labelled(ui, label, colors.warn, "", |ui| {
        let field = Field {
            name: label,
            unit,
            width: theme::FIELD_W,
            flagged: false,
        };
        if boxed_field(ui, &field, value, speed, range, Some(decimals)) {
            carried = Carried::Edited;
        }
        let back = tr!(
            "field-back-to",
            value = format!("{was:.decimals$}"),
            unit = unit
        );
        if icon_button(ui, icon::RESET, &back).clicked() {
            carried = Carried::Reverted;
        }
    });
    carried
}

/// A named whole number, laid out like `number_row`: pixel counts and layer counts.
pub fn count_row(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    unit: &str,
    speed: f64,
    range: RangeInclusive<u32>,
) -> bool {
    let field = Field {
        name: label,
        unit,
        width: theme::FIELD_W,
        flagged: false,
    };
    row(ui, label, |ui| {
        boxed_field(ui, &field, value, speed, range, None)
    })
}

/// A `dropdown` under a `field_label` of its own, for a choice too long for a `FIELD_W` box.
pub fn picker_row(ui: &mut Ui, label: &str, glyph: &str, chosen: &str) -> egui::Response {
    field_label(ui, label, theme::colors().text_mid, "");
    ui.add_space(4.0);
    let face = crate::ui::widgets::DropdownFace::plain(glyph, chosen);
    crate::ui::widgets::dropdown(ui, &face, ui.available_width())
}

pub(in crate::ui::widgets) fn row(
    ui: &mut Ui,
    label: &str,
    field: impl FnOnce(&mut Ui) -> bool,
) -> bool {
    labelled(ui, label, theme::colors().text_mid, "", field)
}

/// A `field_label` and what `right` lays out from the row's right edge; `right` goes first so
/// a long label is cut to the room it leaves rather than pushing it past the card.
fn labelled<R>(
    ui: &mut Ui,
    label: &str,
    color: Color32,
    unit: &str,
    right: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let inner = right(ui);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                field_label(ui, label, color, unit);
            });
            inner
        })
        .inner
    })
    .inner
}
