//! The Settings window: the support profiles, the presets of the Hollow and Drain tools,
//! how the window behaves and the updates, a page each down its left. It is drawn in the
//! frame the Machine and resin window is, and every edit is written as it is made. See
//! `docs/decisions/0233` and `docs/design/profiles.md`.

mod compensation;
mod confirm;
mod frame;
mod general;
pub(super) mod machines;
mod presets;
mod printer;
mod resin;
mod supports;
mod updates;

use egui::{Align, Layout, RichText, Ui, vec2};
use printer_profiles::Kind;

use crate::files::{self, Handed, Wanted};
use crate::panels::Window;
use crate::settings::{Deleting, Node, Section};
use crate::state::Machine;
use crate::status::Status;
use crate::ui::{focus_ring, hairline, icon, inline_button, theme};
use frame::{child, scrolled, with_header};

/// The window, while it is open, and the question over it.
pub fn window(ctx: &egui::Context, window: &mut Window) {
    if !window.machine.settings.open {
        return;
    }
    let close = frame::show(
        ctx,
        "settings",
        icon::SETTINGS,
        "Settings",
        |ui, list, side| {
            pages(&mut child(ui, list.shrink2(vec2(12.0, 14.0))), window);
            page(ui, side, window);
        },
    );
    if close {
        window.machine.settings.close();
        return;
    }
    ask(ctx, window);
}

/// The pages down the left, the profiles over the window's own settings, and the window
/// and the sheet the rest of the setup lives in.
fn pages(ui: &mut Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing.y = 2.0;
    let settings = &mut window.machine.settings;
    for group in [&Section::PROFILES[..], &Section::OWN[..]] {
        for section in group {
            let active = settings.section == *section;
            if nav_row(ui, section_icon(*section), section.label(), active).clicked() {
                settings.turn_to(*section);
            }
        }
        ui.add_space(10.0);
        hairline(ui);
        ui.add_space(10.0);
    }
    if nav_row(ui, icon::PRINTER, "Machine and resin", false).clicked() {
        window.machine.settings.close();
        crate::panels::open_machines(window.machine);
    }
    // The keys are a sheet rather than a page: there is nothing to edit, and one listing
    // is enough. See `crate::shortcuts`.
    if nav_row(ui, icon::KEYBOARD, "Shortcuts", false).clicked() {
        window.machine.settings.close();
        window.view.options.sheet = true;
    }
}

fn section_icon(section: Section) -> &'static str {
    match section {
        Section::Supports => icon::SUPPORTS,
        Section::Hollow => icon::HOLLOW,
        Section::Drain => icon::DRAIN,
        Section::General => icon::GENERAL,
        Section::Updates => icon::UPDATE,
    }
}

/// The open page under its name, and what the name's row offers for it.
fn page(ui: &mut Ui, side: egui::Rect, window: &mut Window) {
    let section = window.machine.settings.section;
    let (mut head, content) = with_header(ui, side);
    let pressed = heading(&mut head, section);
    match section {
        Section::Supports => supports::page(ui, content, window),
        Section::Hollow | Section::Drain => presets::page(ui, content, window),
        Section::General => scrolled(&mut child(ui, content), |ui| {
            general::form(ui, window.machine);
        }),
        Section::Updates => scrolled(&mut child(ui, content), |ui| {
            updates::form(ui, &mut window.machine.updates);
        }),
    }
    match pressed {
        Some(Heading::Import(kind)) => ask_for_profile(window, kind),
        Some(Heading::New) => supports::add(window.machine, None),
        None => {}
    }
}

/// What the row of a page's name was pressed for.
enum Heading {
    Import(Kind),
    New,
}

/// The page's name on the left, and on a page of profiles the ways to add one.
fn heading(ui: &mut Ui, section: Section) -> Option<Heading> {
    let mut pressed = None;
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new(section.label())
                .font(theme::page_title(18.0))
                .color(theme::colors().text_high),
        );
        let Some(kind) = section.kind() else {
            return;
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if kind == Kind::Support
                && inline_button(ui, icon::ADD, "New profile", false, true).clicked()
            {
                pressed = Some(Heading::New);
            }
            if inline_button(ui, icon::IMPORT, "Import", false, true).clicked() {
                pressed = Some(Heading::Import(kind));
            }
        });
    });
    pressed
}

/// One entry of the list of pages: a glyph and a name, washed while it is the page open.
pub(super) fn nav_row(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool) -> egui::Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::BUTTON_H + 4.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
    });
    let (fill, text) = match (active, response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (egui::Color32::TRANSPARENT, colors.text_mid),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let glyph_at = rect.left_center() + vec2(12.0, 0.0);
    painter.text(
        glyph_at,
        egui::Align2::LEFT_CENTER,
        glyph,
        theme::icon(15.0),
        text,
    );
    let label_at = rect.left_center() + vec2(36.0, 0.0);
    painter.text(
        label_at,
        egui::Align2::LEFT_CENTER,
        label,
        theme::body(),
        text,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// Asks for a profile's file and takes it in. A browser answers on a later frame, through
/// [`import_profile`].
pub(super) fn ask_for_profile(window: &mut Window, kind: Kind) {
    if let Some(file) = files::pick(Wanted::Profile(kind)) {
        import_profile(window, kind, &file);
    }
}

/// Takes `file` in as a profile of `kind` and opens it where it was asked for from: a
/// resin goes on the machine the Machine and resin window has open.
pub fn import_profile(window: &mut Window, kind: Kind, file: &Handed) {
    let machine = &mut *window.machine;
    let printer = match kind {
        Kind::Resin => machine
            .settings
            .node
            .as_ref()
            .map(|node| node.printer().to_owned()),
        _ => None,
    };
    let Some(id) = crate::exchange::import_file(machine, kind, file, printer.as_deref()) else {
        return;
    };
    let settings = &mut machine.settings;
    let catalogue = &machine.slicing.catalogue;
    match (kind, printer) {
        (Kind::Printer, _) => {
            settings.library = None;
            settings.pick(catalogue, Node::Printer(id));
        }
        (Kind::Resin, Some(printer)) => {
            settings.editing_resin = false;
            settings.pick(catalogue, Node::Resin { printer, resin: id });
        }
        (Kind::Support, _) => {
            settings.editing_support = false;
            settings.pick_support(catalogue, &id);
        }
        (Kind::Hollow | Kind::Drain, _) => settings.preset = Some(id),
        (Kind::Resin, None) => {}
    }
}

/// Writes the profile of `kind` under `id` out to a file the user names.
pub(super) fn export_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let text = machine.slicing.catalogue.export(kind, id);
    let Some(text) = report(&mut machine.status, "cannot export the profile", text) else {
        return;
    };
    let name = crate::exchange::file_name(id);
    match files::save_text(&name, Wanted::Profile(kind), &text) {
        Ok(Some(path)) => machine.status = Status::Info(format!("Exported {}", path.display())),
        Ok(None) => {}
        Err(error) => machine.status = Status::failed(&error),
    }
}

/// The question over the window, while a deletion is waiting to be answered for.
fn ask(ctx: &egui::Context, window: &mut Window) {
    let settings = &mut window.machine.settings;
    let Some(deleting) = settings.confirm.clone() else {
        return;
    };
    let Some(answer) = confirm::ask(ctx, &window.machine.slicing.catalogue, &deleting) else {
        return;
    };
    settings.confirm = None;
    if let (confirm::Answer::Confirmed, Deleting::Profile(kind, id)) = (answer, deleting) {
        forget_profile(window.machine, kind, &id);
    }
}

/// Throws away a support profile or a tool's preset, and leaves nothing of it open.
fn forget_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let catalogue = &mut machine.slicing.catalogue;
    let outcome = catalogue.forget_user_copy(kind, id);
    report(&mut machine.status, "cannot delete the profile", outcome);
    let settings = &mut machine.settings;
    settings.editing_support = false;
    settings.preset = None;
    if kind == Kind::Support {
        let first = catalogue.supports().next().map(|entry| entry.id.clone());
        settings.support = None;
        if let Some(first) = first {
            settings.pick_support(catalogue, &first);
        }
    }
}

/// Room above and below the fields of a block.
const BLOCK_PAD: f32 = 18.0;

/// One category of a form: its title in a column of its own beside its fields, over a
/// hairline. Where the form is too narrow for both, the title stands over the fields.
pub(super) fn form_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let title_text = egui::RichText::new(title)
        .font(theme::block_title())
        .color(theme::colors().text_mid);
    let beside = theme::BLOCK_TITLE_W + theme::BLOCK_GAP + theme::FIELD_W * 2.0;
    ui.add_space(BLOCK_PAD);
    let inner = if ui.available_width() < beside {
        ui.label(title_text);
        ui.add_space(4.0);
        add(ui)
    } else {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(theme::BLOCK_TITLE_W, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(theme::BLOCK_TITLE_W);
                    ui.add_space(4.0);
                    ui.label(title_text);
                },
            );
            ui.add_space(theme::BLOCK_GAP);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width().min(theme::BLOCK_FIELDS_W));
                ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                add(ui)
            })
            .inner
        })
        .inner
    };
    ui.add_space(BLOCK_PAD);
    hairline(ui);
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
            let field = crate::ui::text_box(egui::TextEdit::singleline(&mut text))
                .font(theme::label())
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
