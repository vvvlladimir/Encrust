//! The Settings screen: the profiles the plate prints with, opened for editing.
//!
//! The screen replaces the plate rather than floating over it, because editing a printer
//! is not something done while dragging a model. Every edit is written as it is made.
//! See `docs/design/profiles.md`.

mod compensation;
mod confirm;
mod machines;
mod printer;
mod resin;
mod supports;
mod updates;

use crate::panels::Window;
use crate::settings::Section;
use crate::state::Machine;
use crate::status::Status;
use crate::ui::{hairline, icon, icon_button, theme};

/// How wide the list of sections down the left is, and the list of profiles beside it.
const SECTIONS_W: f32 = 196.0;
pub(super) const LIST_W: f32 = 290.0;

/// How wide the form grows: one column of fields reads best at a form's width, and the
/// support form sets two of them side by side.
const FORM_W: f32 = 680.0;
const WIDE_FORM_W: f32 = 980.0;

/// Room between the cards of a page, and inside each.
const CARD_GAP: f32 = 12.0;
const CARD_MARGIN: egui::Margin = egui::Margin::symmetric(16, 14);

pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::Panel::left("settings-sections")
        .exact_size(SECTIONS_W)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(theme::colors().base)
                .inner_margin(egui::Margin::symmetric(10, 14)),
        )
        .show(ui, |ui| sections(ui, window));

    egui::Panel::top("settings-title")
        .frame(
            egui::Frame::new()
                .fill(theme::colors().panel)
                .inner_margin(egui::Margin::symmetric(20, 14)),
        )
        .show(ui, |ui| title(ui, window.machine));

    // Updates is one page with nothing to pick from, so it has no list.
    if window.machine.settings.section != Section::Updates {
        egui::Panel::left("settings-list")
            .exact_size(LIST_W)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::colors().panel)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match window.machine.settings.section {
                        Section::Printers => machines::list(ui, window),
                        Section::Supports => supports::list(ui, window.machine),
                        Section::Updates => {}
                    });
            });
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::colors().panel))
        .show(ui, |ui| form(ui, window));
}

/// The pages of the screen, and the sheet of keys under them.
fn sections(ui: &mut egui::Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.label(
        egui::RichText::new("Settings")
            .font(theme::section())
            .color(theme::colors().text_low),
    );
    ui.add_space(8.0);
    for section in Section::ALL {
        let active = window.machine.settings.section == section;
        if nav_row(ui, section_icon(section), section.label(), active).clicked() {
            window.machine.settings.section = section;
        }
    }
    ui.add_space(10.0);
    hairline(ui);
    ui.add_space(10.0);
    // The keys are a sheet rather than a page: there is nothing to edit, and one listing
    // is enough. See `crate::shortcuts`.
    if nav_row(ui, icon::KEYBOARD, "Shortcuts", false).clicked() {
        window.view.options.sheet = true;
    }
}

fn section_icon(section: Section) -> &'static str {
    match section {
        Section::Printers => icon::PRINTER,
        Section::Supports => icon::SUPPORTS,
        Section::Updates => icon::UPDATE,
    }
}

/// One entry of the list of pages: a glyph and a name, washed while it is the page open.
fn nav_row(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool) -> egui::Response {
    let colors = theme::colors();
    let size = egui::vec2(ui.available_width(), 32.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let (fill, text) = match (active, response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (egui::Color32::TRANSPARENT, colors.text_mid),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let glyph_at = rect.left_center() + egui::vec2(12.0, 0.0);
    painter.text(
        glyph_at,
        egui::Align2::LEFT_CENTER,
        glyph,
        theme::icon(15.0),
        text,
    );
    let label_at = rect.left_center() + egui::vec2(36.0, 0.0);
    painter.text(
        label_at,
        egui::Align2::LEFT_CENTER,
        label,
        theme::label(),
        text,
    );
    response
}

/// The form of what the list has picked, in a column of its own width.
fn form(ui: &mut egui::Ui, window: &mut Window) {
    let widest = match window.machine.settings.section {
        Section::Supports => WIDE_FORM_W,
        Section::Printers | Section::Updates => FORM_W,
    };
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = (ui.available_width() - 40.0).min(widest);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                    match window.machine.settings.section {
                        Section::Printers => machines::form(ui, window),
                        Section::Supports => supports::form(ui, window),
                        Section::Updates => updates::form(ui, &mut window.machine.updates),
                    }
                    ui.add_space(24.0);
                });
            });
        });
}

/// The page's name, and the way back to the plate.
fn title(ui: &mut egui::Ui, machine: &mut Machine) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(machine.settings.section.label())
                .font(egui::FontId::new(
                    18.0,
                    egui::FontFamily::Name(theme::SEMIBOLD.into()),
                ))
                .color(theme::colors().text_high),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, icon::CANCEL, "Close settings").clicked() {
                machine.settings.open = false;
            }
        });
    });
}

/// One category of a page: a titled card the fields of that category sit in.
/// The compensation calculator floating over the Settings screen, when one is open.
pub(super) fn calculators(ctx: &egui::Context, machine: &mut crate::state::Machine) {
    if !machine.settings.open {
        return;
    }
    let settings = &mut machine.settings;
    let Some(resin) = settings.resin.as_mut() else {
        settings.calculators.open = None;
        return;
    };
    compensation::calculator(
        ctx,
        &mut settings.calculators,
        &mut resin.draft.values.compensation,
    );
}

/// The question over the Settings screen, while a deletion is waiting to be answered for.
pub(super) fn confirm(ctx: &egui::Context, window: &mut Window) {
    // A question nobody answered does not follow the user back to the plate.
    if !window.machine.settings.open {
        window.machine.settings.confirm = None;
        return;
    }
    let Some(deleting) = window.machine.settings.confirm.clone() else {
        return;
    };
    let Some(answer) = confirm::ask(ctx, &window.machine.slicing.catalogue, &deleting) else {
        return;
    };
    window.machine.settings.confirm = None;
    if matches!(answer, confirm::Answer::Confirmed) {
        machines::delete(window, deleting);
    }
}

pub(super) fn form_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner = egui::Frame::new()
        .fill(theme::colors().base)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_SURFACE)
        .inner_margin(CARD_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .font(theme::section())
                    .color(theme::colors().text_mid),
            );
            ui.add_space(6.0);
            add(ui)
        })
        .inner;
    ui.add_space(CARD_GAP);
    inner
}

/// A name that is taken when Enter is pressed or the field is left, rather than on every
/// key: renaming may split a resin off, and that should happen once.
pub(super) fn name_row(ui: &mut egui::Ui, salt: &str, name: &str) -> Option<String> {
    let id = ui.id().with(("name", salt));
    let mut text = ui.data(|data| {
        data.get_temp::<String>(id)
            .unwrap_or_else(|| name.to_owned())
    });
    let mut response = None;
    ui.horizontal(|ui| {
        crate::ui::field_label(ui, "Name", theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let field = egui::TextEdit::singleline(&mut text)
                .font(theme::label())
                .vertical_align(egui::Align::Center)
                .margin(egui::Margin::symmetric(8, 0))
                .background_color(theme::colors().raised);
            response = Some(ui.add_sized(egui::vec2(ui.available_width(), theme::FIELD_H), field));
        });
    });
    let response = response?;
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let trimmed = text.trim();
    (response.lost_focus() && !trimmed.is_empty() && trimmed != name).then(|| trimmed.to_owned())
}

/// Whether an edit may be written now: not while a value is still being dragged.
pub(super) fn settled(ui: &egui::Ui) -> bool {
    !ui.input(|input| input.pointer.any_down())
}

/// Says what went wrong when a profile could not be written or changed.
pub(super) fn report<T>(
    status: &mut Status,
    what: &str,
    outcome: Result<T, printer_profiles::ProfileError>,
) -> Option<T> {
    match outcome {
        Ok(value) => Some(value),
        Err(error) => {
            *status = Status::failed(&anyhow::Error::new(error).context(what.to_owned()));
            None
        }
    }
}
