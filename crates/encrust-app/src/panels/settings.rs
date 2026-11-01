//! The Settings window: support profiles, Hollow and Drain presets, behaviour and updates,
//! a page each. Edits are written as made; see `docs/decisions/0233`, `docs/design/profiles.md`.

mod appearance;
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
use crate::i18n::tr;
use crate::panels::Window;
use crate::settings::{Node, Section, Tab};
use crate::state::Machine;
use crate::status::Status;
use crate::ui::{
    chooser_group, focus_ring, hairline, hover_mix, icon, icon_button, inline_button, theme,
};
use frame::{child, scrolled, with_header};

/// The window, while it is open, and the question over it, which may be asked without it.
pub fn window(ctx: &egui::Context, window: &mut Window) {
    if window.machine.settings.open {
        let close = frame::show(
            ctx,
            "settings",
            icon::SETTINGS,
            &tr!("settings-title"),
            |ui, list, side| {
                pages(&mut child(ui, list.shrink2(vec2(12.0, 14.0))), window);
                page(ui, side, window);
            },
        );
        if close {
            window.machine.settings.close();
            return;
        }
    }
    ask(ctx, window);
}

/// The pages down the left under their group names, narrowed by the line over them: the
/// profiles, the window's own, then help; at the list's foot the window the rest lives in.
fn pages(ui: &mut Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing.y = 2.0;
    find_line(ui, &mut window.machine.settings.find);
    ui.add_space(FIND_GAP);
    let find = window.machine.settings.find.to_lowercase();
    let groups = [
        (tr!("settings-group-profiles"), &Section::PROFILES[..]),
        (tr!("settings-group-window"), &Section::OWN[..]),
    ];
    for (index, (name, group)) in groups.into_iter().enumerate() {
        let shown: Vec<Section> = group
            .iter()
            .copied()
            .filter(|section| matches(*section, &find))
            .collect();
        if shown.is_empty() {
            continue;
        }
        if index > 0 {
            ui.add_space(GROUP_GAP);
        }
        chooser_group(ui, &name);
        for section in shown {
            let active = window.machine.settings.section == section;
            let aside = aside_of(window, section);
            if nav_row(ui, section_icon(section), &section.label(), active, aside).clicked() {
                window.machine.settings.turn_to(section);
            }
        }
    }
    // The keys are a sheet rather than a page: there is nothing to edit, and one listing
    // is enough. See `crate::shortcuts`.
    let keys = tr!("settings-shortcuts");
    if keys.to_lowercase().contains(&find) {
        ui.add_space(GROUP_GAP);
        chooser_group(ui, &tr!("settings-group-help"));
        if nav_row(ui, icon::KEYBOARD, &keys, false, NavAside::Elsewhere).clicked() {
            window.machine.settings.close();
            window.view.options.sheet = true;
        }
    }
    ui.add_space((ui.available_height() - NAV_ROW_H).max(0.0));
    let machines = tr!("machines-title");
    if nav_row(ui, icon::PRINTER, &machines, false, NavAside::Elsewhere).clicked() {
        window.machine.settings.close();
        crate::panels::open_machines(window.machine);
    }
}

/// The line that narrows the pages, `⌘F` reaching it from anywhere in the window.
fn find_line(ui: &mut Ui, find: &mut String) {
    let keys = format!("{}F", icon::KEY_COMMAND);
    let field = ui.add(
        crate::ui::text_box(egui::TextEdit::singleline(find))
            .hint_text(format!("{}  {}", icon::FIND, tr!("settings-find")))
            .desired_width(ui.available_width()),
    );
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
        field.request_focus();
    }
    if find.is_empty() {
        let right = field.rect.right() - FIND_CAP_INSET;
        crate::ui::keycap(ui.painter(), &keys, right, field.rect.center().y);
    }
}

/// Whether a page answers what was typed: by its name, or by a setting it holds.
fn matches(section: Section, find: &str) -> bool {
    if find.is_empty() {
        return true;
    }
    let words = match section {
        Section::Appearance => tr!("settings-words-appearance"),
        Section::General => tr!("settings-words-general"),
        Section::Updates => tr!("settings-words-updates"),
        Section::Supports | Section::Hollow | Section::Drain => String::new(),
    };
    format!("{} {words}", section.label())
        .to_lowercase()
        .contains(find)
}

/// What a page's row carries at its right: how many profiles it lists, or that a release
/// waits on Updates.
fn aside_of(window: &Window, section: Section) -> NavAside {
    let catalogue = &window.machine.slicing.catalogue;
    match section {
        Section::Supports => {
            let by_hand = catalogue
                .supports()
                .filter(|entry| !catalogue.is_auto_shape(&entry.id))
                .count();
            NavAside::Count(catalogue.autos().count() + by_hand)
        }
        Section::Hollow => NavAside::Count(catalogue.presets(Kind::Hollow).count()),
        Section::Drain => NavAside::Count(catalogue.presets(Kind::Drain).count()),
        Section::Updates if window.machine.updates.waiting().is_some() => NavAside::Waiting,
        Section::Appearance | Section::General | Section::Updates => NavAside::None,
    }
}

fn section_icon(section: Section) -> &'static str {
    match section {
        Section::Supports => icon::SUPPORTS,
        Section::Hollow => icon::HOLLOW,
        Section::Drain => icon::DRAIN,
        Section::Appearance => icon::APPEARANCE,
        Section::General => icon::GENERAL,
        Section::Updates => icon::UPDATE,
    }
}

/// The open page under its name, and what the name's row offers for it.
fn page(ui: &mut Ui, side: egui::Rect, window: &mut Window) {
    let section = window.machine.settings.section;
    let (mut head, content) = with_header(ui, side);
    let pressed = match supports::editing(&window.machine.settings) {
        Some(name) if section == Section::Supports => form_heading(&mut head, &name),
        _ => heading(&mut head, section),
    };
    match section {
        Section::Supports => supports::page(ui, content, window),
        Section::Hollow | Section::Drain => presets::page(ui, content, window),
        Section::Appearance => scrolled(&mut child(ui, content), |ui| {
            appearance::form(ui, window.machine);
        }),
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
        Some(Heading::Back) => window.machine.settings.editing_support = false,
        None => {}
    }
}

/// What the row of a page's name was pressed for.
enum Heading {
    Import(Kind),
    New,
    Back,
}

/// The way back to every profile, then the name of the one open in the form and what kind
/// of profile it is.
fn form_heading(ui: &mut Ui, name: &str) -> Option<Heading> {
    ui.horizontal_centered(|ui| {
        let back = icon_button(ui, icon::BACK, &tr!("profiles-every")).clicked();
        ui.label(
            RichText::new(name)
                .font(theme::page_title(theme::PAGE_TITLE_PT))
                .color(theme::colors().text_high),
        );
        ui.label(
            RichText::new(tr!("settings-support-profile"))
                .font(theme::body())
                .color(theme::colors().text_low),
        );
        back.then_some(Heading::Back)
    })
    .inner
}

/// The page's name on the left, beside it on Updates the build this is, and on a page of
/// profiles the ways to add one.
fn heading(ui: &mut Ui, section: Section) -> Option<Heading> {
    let mut pressed = None;
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new(section.label())
                .font(theme::page_title(theme::PAGE_TITLE_PT))
                .color(theme::colors().text_high),
        );
        if section == Section::Updates {
            let running = crate::updates::Updates::running().to_string();
            ui.label(
                RichText::new(tr!("updates-this-build", version = running))
                    .font(theme::page_title(theme::PAGE_TITLE_PT))
                    .color(theme::colors().text_low),
            );
        }
        let Some(kind) = section.kind() else {
            return;
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if kind == Kind::Support
                && inline_button(ui, icon::ADD, &tr!("settings-new-profile"), false, true).clicked()
            {
                pressed = Some(Heading::New);
            }
            if inline_button(ui, icon::IMPORT, &tr!("settings-import"), false, true).clicked() {
                pressed = Some(Heading::Import(kind));
            }
        });
    });
    pressed
}

const NAV_ROW_H: f32 = theme::BUTTON_H + 4.0;
/// Room over a group name of the list but the first, and under the find line.
const GROUP_GAP: f32 = 10.0;
const FIND_GAP: f32 = 12.0;
/// From the find line's right edge to its keycap's, points.
const FIND_CAP_INSET: f32 = 6.0;
/// The badge on a page a release waits on, points across.
const BADGE_D: f32 = 16.0;

/// What stands at the right of a page's row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NavAside {
    None,
    /// How many profiles the page lists.
    Count(usize),
    /// A release waits to be installed.
    Waiting,
    /// The row opens something other than a page of this window.
    Elsewhere,
}

/// One entry of the list of pages: a glyph and a name, washed while it is the page open,
/// and at its right what [`NavAside`] says.
pub(super) fn nav_row(
    ui: &mut egui::Ui,
    glyph: &str,
    label: &str,
    active: bool,
    aside: NavAside,
) -> egui::Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), NAV_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
    });
    let (fill, text) = match active {
        true => (colors.accent_wash, colors.accent_soft),
        false => (
            hover_mix(ui, &response, egui::Color32::TRANSPARENT, colors.hover),
            hover_mix(ui, &response, colors.text_mid, colors.text_high),
        ),
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
    let right = rect.right_center() - vec2(12.0, 0.0);
    match aside {
        NavAside::None => {}
        NavAside::Count(count) => {
            let ink = if active {
                colors.accent_soft
            } else {
                colors.text_low
            };
            let at = egui::Align2::RIGHT_CENTER;
            painter.text(right, at, count.to_string(), theme::figures(12.0), ink);
        }
        NavAside::Waiting => {
            let centre = right - vec2(BADGE_D / 2.0, 0.0);
            painter.circle_filled(centre, BADGE_D / 2.0, colors.accent);
            let at = egui::Align2::CENTER_CENTER;
            painter.text(centre, at, "1", theme::figures(11.0), colors.on_accent);
        }
        NavAside::Elsewhere => {
            let at = egui::Align2::RIGHT_CENTER;
            painter.text(
                right,
                at,
                icon::ELSEWHERE,
                theme::icon(13.0),
                colors.text_low,
            );
        }
    }
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
            settings.pick(catalogue, Node::Printer(id.clone()));
            machines::fill_empty_plate(window, &id);
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
        (Kind::Resin, None) | (Kind::Auto, _) => {}
    }
    // A plate with no resin takes the first one its machine now has.
    if matches!(kind, Kind::Printer | Kind::Resin) {
        window.machine.slicing.reload_resin();
    }
}

/// Takes a file dropped on the Machine and resin window in as a profile: a resin when the
/// window shows a machine's resins or the file can be nothing else, a machine otherwise.
pub fn import_dropped(window: &mut Window, file: &Handed) {
    let settings = &window.machine.settings;
    let resins_in_view =
        settings.library.is_none() && settings.node.is_some() && settings.tab == Tab::Resins;
    let path = file.path();
    let only_a_resin =
        printer_profiles::ForeignResin::reads(path) || core_pipeline::reads_sliced_file(path);
    let kind = match resins_in_view || only_a_resin {
        true => Kind::Resin,
        false => Kind::Printer,
    };
    import_profile(window, kind, file);
}

/// Writes the profile of `kind` under `id` out to a file the user names.
pub(super) fn export_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let text = machine.slicing.catalogue.export(kind, id);
    let Some(text) = report(&mut machine.status, &tr!("settings-cannot-export"), text) else {
        return;
    };
    let name = crate::exchange::file_name(id);
    match files::save_text(&name, Wanted::Profile(kind), &text) {
        Ok(Some(path)) => {
            machine.status =
                Status::Info(tr!("status-exported", path = path.display().to_string()));
        }
        Ok(None) => {}
        Err(error) => machine.status = Status::failed(&error),
    }
}

/// The question over the window, while the profiles that would not read wait to be given up.
fn ask(ctx: &egui::Context, window: &mut Window) {
    if !window.machine.settings.reset_unreadable {
        return;
    }
    let Some(answer) = confirm::ask(ctx, &window.machine.slicing.catalogue) else {
        return;
    };
    window.machine.settings.reset_unreadable = false;
    if matches!(answer, confirm::Answer::Confirmed) {
        let machine = &mut *window.machine;
        let outcome = machine.slicing.catalogue.discard_unreadable();
        report(&mut machine.status, &tr!("settings-cannot-reset"), outcome);
    }
}

/// Throws away a support profile or a tool's preset, and leaves nothing of it open.
pub(super) fn forget_profile(machine: &mut Machine, kind: Kind, id: &str) {
    let catalogue = &mut machine.slicing.catalogue;
    let outcome = catalogue.forget_user_copy(kind, id);
    report(&mut machine.status, &tr!("settings-cannot-delete"), outcome);
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

/// One category of a form: its title in a column beside its fields over a hairline, or above
/// them when narrow. What the fields `describe` is a question mark beside the title.
pub(super) fn form_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    switched_card(ui, title, None, add)
}

/// A form card whose title carries the switch turning its fields on, where `on` is given.
pub(super) fn switched_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    on: Option<&mut bool>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner = card_cell(ui, title, on, add);
    close_cards(ui);
    inner
}

/// A card's title and fields, open below for [`close_cards`].
fn card_cell<R>(
    ui: &mut egui::Ui,
    title: &str,
    on: Option<&mut bool>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let id = ui.id().with(("form-card", title));
    let texts = crate::ui::described(ui, id);
    let beside = theme::BLOCK_TITLE_W + theme::BLOCK_GAP + theme::FIELD_W * 2.0;
    ui.add_space(BLOCK_PAD);
    if ui.available_width() < beside {
        card_title(ui, title, &texts, on);
        ui.add_space(4.0);
        crate::ui::describing(ui, id, add)
    } else {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(theme::BLOCK_TITLE_W, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(theme::BLOCK_TITLE_W);
                    ui.add_space(4.0);
                    card_title(ui, title, &texts, on);
                },
            );
            ui.add_space(theme::BLOCK_GAP);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width().min(theme::BLOCK_FIELDS_W));
                ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                crate::ui::describing(ui, id, add)
            })
            .inner
        })
        .inner
    }
}

fn close_cards(ui: &mut egui::Ui) {
    ui.add_space(BLOCK_PAD);
    hairline(ui);
}

/// A form card's title, the question mark that reads out what its fields described, and at
/// the right the card's switch when it has one.
fn card_title(ui: &mut egui::Ui, title: &str, texts: &[String], on: Option<&mut bool>) {
    let title_text = egui::RichText::new(title)
        .font(theme::block_title())
        .color(theme::colors().text_mid);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(title_text);
        if !texts.is_empty() {
            crate::ui::help_mark(ui, texts);
        }
        if let Some(on) = on {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                crate::ui::bare_switch(ui, on, title);
            });
        }
    });
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
        crate::ui::row_name(ui, &tr!("settings-name"));
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
