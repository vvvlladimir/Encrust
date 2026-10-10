//! The Machine and resin window: the machines down its left, and the resins, the profile
//! and the network of the one picked in tabs beside them. A printer's resins are its own
//! presets, and any resin ever added to a printer is in the pool for the others. See
//! `docs/decisions/0220`.

use egui::{Align, Color32, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use printer_profiles::{Catalogue, Connection, Entry, Kind, PrinterProfile, ProfileError};

mod library;
mod resins;

use crate::panels::Window;
use crate::panels::settings::{compensation, confirm, printer, report};
use crate::settings::{
    Deleting, Library, Node, Tab, add_resin, delete_resin, duplicate_resin, installed, new_printer,
    new_resin, take_resin_off,
};
use crate::ui::{dialog_frame, dialog_head, icon, inline_button, quiet_button, theme, two_lines};

/// The most of the screen the window takes, where the screen has it.
const MOST: egui::Vec2 = vec2(1080.0, 720.0);
/// The machine's name and the tabs over the right side, and the action bar under it.
const HEADER_H: f32 = 64.0;
const FOOTER_H: f32 = 52.0;
/// Room either side of what the right side shows.
const SIDE_PAD: f32 = 20.0;

/// What a click in the window asked for, carried out once the window has been drawn.
enum Action {
    Pick(Node),
    /// A resin opened in its form, in the table's place.
    Edit(Node),
    NewPrinter,
    /// A machine taken out of the library, which is a copy of it in the user's directory.
    InstallPrinter(String),
    NewResin(String),
    AddResin {
        printer: String,
        resin: String,
    },
    DuplicateResin {
        printer: String,
        resin: String,
    },
    /// The plate is printed on this machine from now on.
    UsePrinter(String),
    /// The plate is printed with this resin, on the machine it is set up on.
    UseResin {
        printer: String,
        resin: String,
    },
    /// The library of every shipped machine, in the tabs' place.
    OpenLibrary,
    /// Something that cannot be undone, which the window asks about before doing it.
    Ask(Deleting),
}

/// The window, while it is open, and the question and the calculator over it.
pub fn window(ctx: &egui::Context, window: &mut Window) {
    if !window.machine.settings.machines {
        return;
    }
    let screen = ctx.content_rect().size() * theme::DIALOG_SHARE;
    let size = vec2(MOST.x.min(screen.x), MOST.y.min(screen.y));
    let mut action = None;
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("machine-and-resin"))
        .frame(dialog_frame())
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            close = body(ui, rect, window, &mut action);
        });
    if let Some(action) = action {
        apply(window, action);
    }
    if close || modal.should_close() {
        window.machine.settings.close_machines();
        return;
    }
    calculators(ctx, window);
    ask(ctx, window);
}

/// Everything inside the window's frame. Answers whether it was asked to close.
fn body(ui: &mut Ui, rect: Rect, window: &mut Window, action: &mut Option<Action>) -> bool {
    let head = rect.with_max_y(rect.top() + theme::DIALOG_HEAD_H);
    let list = Rect::from_min_max(
        pos2(rect.left(), head.bottom()),
        pos2(rect.left() + theme::DIALOG_LIST_W, rect.bottom()),
    );
    let side = Rect::from_min_max(pos2(list.right() + 1.0, head.bottom()), rect.max);
    let mut title = ui.new_child(
        UiBuilder::new()
            .max_rect(head.shrink2(vec2(16.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let close = dialog_head(&mut title, icon::PRINTER, "Machine and resin");
    let painter = ui.painter();
    painter.hline(rect.x_range(), head.bottom(), stroke());
    painter.vline(list.right(), list.y_range(), stroke());
    machine_list(&mut child(ui, list), window, action);
    match window.machine.settings.library.is_some() {
        true => shop(
            &mut child(ui, side.shrink2(vec2(SIDE_PAD, 16.0))),
            window,
            action,
        ),
        false => machine(ui, side, window, action),
    }
    close
}

/// A child laid out top down inside `rect`, and clipped to it.
fn child(ui: &mut Ui, rect: Rect) -> Ui {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
}

fn stroke() -> egui::Stroke {
    egui::Stroke::new(1.0, theme::colors().hairline)
}

/// The machines the user installed, by brand, over a search line, and the way to the
/// library under them. The shipped catalogue is that library; see `docs/decisions/0158`.
fn machine_list(ui: &mut Ui, window: &mut Window, action: &mut Option<Action>) {
    let settings = &mut window.machine.settings;
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(&mut settings.search)
                .hint_text(format!("{}  Your machines", icon::FIND))
                .margin(egui::Margin::symmetric(10, 8))
                .desired_width(ui.available_width() - 12.0),
        );
    });
    ui.add_space(8.0);
    let footer_top = ui.max_rect().bottom() - FOOTER_H - 30.0;
    let rows = Rect::from_min_max(ui.cursor().min, pos2(ui.max_rect().right(), footer_top));
    let catalogue = &window.machine.slicing.catalogue;
    let open = settings
        .node
        .as_ref()
        .map(Node::printer)
        .unwrap_or_default();
    let in_hand = window.machine.slicing.printer_id.as_deref();
    let mut list = child(ui, rows);
    let scroll = egui::ScrollArea::vertical().id_salt("machine-list");
    scroll.auto_shrink([false, false]).show(&mut list, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let listed = mine(catalogue, &settings.search);
        if listed.is_empty() {
            empty_list(ui, settings.search.trim().is_empty());
        }
        for entry in listed {
            let picked = settings.library.is_none() && entry.id == open;
            if machine_row(ui, entry, picked, in_hand == Some(entry.id.as_str())).clicked() {
                *action = Some(Action::Pick(Node::Printer(entry.id.clone())));
            }
        }
    });
    let foot = Rect::from_min_max(pos2(ui.max_rect().left(), footer_top), ui.max_rect().max);
    ui.painter().hline(foot.x_range(), foot.top(), stroke());
    add_machine(&mut child(ui, foot.shrink(12.0)), catalogue, action);
}

fn empty_list(ui: &mut Ui, unsearched: bool) {
    let text = match unsearched {
        true => "No machine yet. Add one from the library below.",
        false => "No machine of that name.",
    };
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        crate::ui::describe(ui, text);
    });
}

/// One machine: its name over its maker and the file it is sliced into, the network glyph
/// when it takes a file over one, and a check on the one the plate is printed on.
fn machine_row(
    ui: &mut Ui,
    entry: &Entry<PrinterProfile>,
    picked: bool,
    in_hand: bool,
) -> egui::Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::TWO_LINE_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = match (picked, response.hovered()) {
        (true, _) => colors.accent_wash,
        (false, true) => colors.hover,
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, fill);
    let profile = &entry.profile;
    let format = core_pipeline::SlicedFormat::from(profile.output).extension();
    let detail = format!("{}, .{format}", profile.manufacturer);
    two_lines(
        &painter,
        rect.left_center() + vec2(14.0, 0.0),
        &profile.name,
        &detail,
    );
    let marks = [
        (in_hand, icon::IN_HAND, colors.accent_soft),
        (
            profile.connection != Connection::None,
            icon::NETWORK,
            colors.ok,
        ),
    ];
    let mut right = rect.right_center() - vec2(16.0, 0.0);
    for (_, glyph, tint) in marks.into_iter().filter(|(shown, ..)| *shown) {
        painter.text(
            right,
            egui::Align2::RIGHT_CENTER,
            glyph,
            theme::icon(13.0),
            tint,
        );
        right.x -= 22.0;
    }
    response
}

/// The way into the library, and how much of it there is.
fn add_machine(ui: &mut Ui, catalogue: &Catalogue, action: &mut Option<Action>) {
    if crate::ui::secondary_button(ui, icon::ADD, "Add a machine").clicked() {
        *action = Some(Action::OpenLibrary);
    }
    let (shipped, makers) = crate::settings::library_size(catalogue);
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(format!("From {shipped} profiles by {makers} makers"))
                .font(theme::small())
                .color(theme::colors().text_low),
        );
    });
}

/// The machine picked in the list: its name and the tabs over what the open tab shows.
fn machine(ui: &mut Ui, side: Rect, window: &mut Window, action: &mut Option<Action>) {
    let Some(draft) = window.machine.settings.printer.as_ref() else {
        let mut ui = child(ui, side.shrink2(vec2(SIDE_PAD, 16.0)));
        crate::ui::describe(&mut ui, "Pick a machine, or add one from the library.");
        return;
    };
    let (name, maker) = (draft.values.name.clone(), draft.values.manufacturer.clone());
    let id = draft.id.clone();
    let header = side.with_max_y(side.top() + HEADER_H);
    let content = Rect::from_min_max(pos2(side.left(), header.bottom() + 1.0), side.max);
    ui.painter()
        .hline(side.x_range(), header.bottom(), stroke());
    let mut head = child(ui, header.shrink2(vec2(SIDE_PAD, 0.0)));
    heading(&mut head, &name, &maker, &mut window.machine.settings.tab);
    match window.machine.settings.tab {
        Tab::Resins => resins::tab(ui, content, window, &id, action),
        Tab::Machine => {
            let (mut body, mut bar) = with_footer(ui, content);
            scrolled(&mut body, |ui| printer::form(ui, window));
            machine_actions(&mut bar, window, &id, action);
        }
        Tab::Network => scrolled(&mut child(ui, content), |ui| printer::network(ui, window)),
    }
}

/// The machine's name and maker on the left, the tabs on the right, underlined while open.
fn heading(ui: &mut Ui, name: &str, maker: &str, tab: &mut Tab) {
    let colors = theme::colors();
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new(name)
                .font(theme::page_title(18.0))
                .color(colors.text_high),
        );
        ui.label(
            RichText::new(maker)
                .font(theme::page_title(18.0))
                .color(colors.text_low),
        );
        ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for each in Tab::ALL.into_iter().rev() {
                if tab_button(ui, each.label(), *tab == each).clicked() {
                    *tab = each;
                }
            }
        });
    });
}

/// One tab: its name, with the accent under it while it is the one open.
fn tab_button(ui: &mut Ui, label: &str, open: bool) -> egui::Response {
    let colors = theme::colors();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::label(), colors.text_high);
    let size = vec2(galley.size().x, theme::BUTTON_H + 4.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
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
    response
}

/// `content` split into what scrolls and the action bar along its foot.
fn with_footer(ui: &mut Ui, content: Rect) -> (Ui, Ui) {
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
fn scrolled(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(SIDE_PAD);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - SIDE_PAD);
                    ui.spacing_mut().item_spacing.y = theme::ITEM_GAP;
                    add(ui);
                    ui.add_space(12.0);
                });
            });
        });
}

/// What the Machine tab's bar offers: taking the machine off, and printing on it.
fn machine_actions(ui: &mut Ui, window: &Window, id: &str, action: &mut Option<Action>) {
    if quiet_button(ui, icon::REMOVE, "Remove this machine", true, true).clicked() {
        *action = Some(Action::Ask(Deleting::Printer(id.to_owned())));
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let in_hand = window.machine.slicing.printer_id.as_deref() == Some(id);
        let label = if in_hand {
            "The plate prints on this"
        } else {
            "Use this machine"
        };
        if inline_button(ui, "", label, !in_hand, !in_hand).clicked() {
            *action = Some(Action::UsePrinter(id.to_owned()));
        }
    });
}

/// The library page, and what a pick in it does: either way the library closes, because
/// what was picked is what the user wants to look at next.
fn shop(ui: &mut Ui, window: &mut Window, action: &mut Option<Action>) {
    let machine = &mut *window.machine;
    let Some(library) = machine.settings.library.as_mut() else {
        return;
    };
    let picked = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            library::page(ui, &machine.slicing.catalogue, library)
        })
        .inner;
    match picked {
        Some(library::Picked::Custom) => *action = Some(Action::NewPrinter),
        Some(library::Picked::Printer(id)) => *action = Some(Action::InstallPrinter(id)),
        None => {}
    }
}

fn apply(window: &mut Window, action: Action) {
    let settings = &mut window.machine.settings;
    // Anything picked anywhere is what the tabs show next, so the library gets out of the
    // way whether the pick came from it or from the list beside it.
    settings.library = None;
    let picked = match action {
        Action::Pick(node) => Some(node),
        Action::Edit(node) => {
            settings.editing_resin = true;
            Some(node)
        }
        Action::OpenLibrary => {
            settings.library = Some(Library::default());
            None
        }
        Action::UsePrinter(id) => return use_printer(window, &id),
        Action::UseResin { printer, resin } => return use_resin(window, &printer, &resin),
        Action::Ask(deleting) => {
            settings.confirm = Some(deleting);
            None
        }
        writing => write(window.machine, writing),
    };
    window.machine.slicing.reload_resin();
    pick(window, picked);
}

/// Carries out an action that writes the user's directory. Answers what to open next.
fn write(machine: &mut crate::state::Machine, action: Action) -> Option<Node> {
    let catalogue = &mut machine.slicing.catalogue;
    let status = &mut machine.status;
    match action {
        Action::NewPrinter => {
            report(status, "cannot add a printer", new_printer(catalogue)).map(Node::Printer)
        }
        Action::InstallPrinter(id) => {
            let outcome = install_printer(catalogue, &id);
            report(status, "cannot add the machine", outcome).map(|()| Node::Printer(id))
        }
        Action::NewResin(printer) => {
            // A new resin is named and measured before it is anything else.
            machine.settings.editing_resin = true;
            let outcome = new_resin(catalogue, &printer);
            report(status, "cannot add a resin", outcome)
                .map(|resin| Node::Resin { printer, resin })
        }
        Action::AddResin { printer, resin } => {
            let outcome = add_resin(catalogue, &printer, &resin);
            report(status, "cannot add the resin", outcome).map(|()| Node::Resin { printer, resin })
        }
        Action::DuplicateResin { printer, resin } => {
            let outcome = duplicate_resin(catalogue, &printer, &resin);
            report(status, "cannot duplicate the resin", outcome)
                .map(|resin| Node::Resin { printer, resin })
        }
        _ => None,
    }
}

/// Stands the plate under machine `id`, unless it already is.
fn use_printer(window: &mut Window, id: &str) {
    if window.machine.slicing.printer_id.as_deref() == Some(id) {
        return;
    }
    let Ok(entry) = window.machine.slicing.catalogue.printer(id) else {
        return;
    };
    let profile = entry.profile.clone();
    crate::profiles::apply_printer(window, profile, Some(id.to_owned()));
}

/// Prints the plate with `resin`, on the machine it was picked under, as the resin menu
/// of the top bar would.
fn use_resin(window: &mut Window, printer: &str, resin: &str) {
    use_printer(window, printer);
    let slicing = &mut window.machine.slicing;
    let Ok(entry) = slicing.catalogue.resin(resin) else {
        return;
    };
    let profile = entry.profile.clone();
    slicing.resin_id = Some(resin.to_owned());
    slicing.set_material(profile);
}

/// Carries out a deletion the question was answered for. Everything here writes the
/// user's directory and cannot be taken back; see `confirm`.
fn delete(window: &mut Window, deleting: Deleting) {
    let picked = match deleting {
        Deleting::Printer(id) => {
            let machine = &mut *window.machine;
            // Where the list goes next is read while the machine is still in it.
            let next = neighbour(&machine.slicing.catalogue, &id);
            let outcome = machine
                .slicing
                .catalogue
                .forget_user_copy(Kind::Printer, &id);
            report(&mut machine.status, "cannot remove the machine", outcome);
            let in_hand = machine.slicing.printer_id.as_deref() == Some(id.as_str());
            if in_hand {
                stand_under(window, next.clone());
            }
            next.map(Node::Printer)
        }
        Deleting::ResinOff { printer, resin } => {
            let machine = &mut *window.machine;
            let outcome = take_resin_off(&mut machine.slicing.catalogue, &printer, &resin);
            report(&mut machine.status, "cannot take the resin off", outcome);
            Some(Node::Printer(printer))
        }
        Deleting::Resin(resin) => {
            let machine = &mut *window.machine;
            let outcome = delete_resin(&mut machine.slicing.catalogue, &resin);
            report(&mut machine.status, "cannot delete the resin", outcome);
            // The pool holds what this printer is not set up with, so what the form has
            // open is never the resin that just went.
            None
        }
    };
    window.machine.slicing.reload_resin();
    pick(window, picked);
}

/// Opens `node` in the form, and leaves the form as it is when nothing was picked.
fn pick(window: &mut Window, node: Option<Node>) {
    let machine = &mut *window.machine;
    if let Some(node) = node {
        machine.settings.pick(&machine.slicing.catalogue, node);
    }
}

/// Stands the plate under the machine the list moved to, or under none when the last one
/// has been removed: the window prints with the machine it has, not with one that is gone
/// (BUG-14).
fn stand_under(window: &mut Window, id: Option<String>) {
    let profile = id
        .as_deref()
        .and_then(|id| window.machine.slicing.catalogue.printer(id).ok())
        .map(|entry| entry.profile.clone());
    match profile {
        Some(profile) => crate::profiles::apply_printer(window, profile, id),
        None => crate::profiles::clear_printer(window),
    }
}

/// What the list moves to when `id` leaves it: the machine under it, or the one over it
/// when it was the last.
fn neighbour(catalogue: &Catalogue, id: &str) -> Option<String> {
    let listed = mine(catalogue, "");
    let at = listed.iter().position(|entry| entry.id == id)?;
    listed
        .get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|over| listed.get(over)))
        .map(|entry| entry.id.clone())
}

/// Takes a machine out of the library: a copy of the shipped profile under its own id,
/// in the user's directory, which is what makes it the user's to edit and to remove.
fn install_printer(catalogue: &mut Catalogue, id: &str) -> Result<(), ProfileError> {
    let entry = catalogue.printer(id)?;
    if installed(&entry.source) {
        return Ok(());
    }
    let profile = entry.profile.clone();
    catalogue.save_printer(id, &profile).map(drop)
}

/// What a machine is called in the list and matched against in the search.
fn label_of(entry: &Entry<PrinterProfile>) -> String {
    format!("{} {}", entry.profile.manufacturer, entry.profile.name)
}

/// Every machine the catalogue knows, in the order a list shows them: what the user has
/// first, then by brand, and only what `search` matches.
fn sorted<'a>(catalogue: &'a Catalogue, search: &str) -> Vec<&'a Entry<PrinterProfile>> {
    let needle = search.trim().to_lowercase();
    let mut entries: Vec<&Entry<PrinterProfile>> = catalogue
        .printers()
        .filter(|entry| needle.is_empty() || label_of(entry).to_lowercase().contains(&needle))
        .collect();
    entries.sort_by_key(|entry| {
        (
            !installed(&entry.source),
            entry.profile.manufacturer.to_lowercase(),
            entry.profile.name.to_lowercase(),
        )
    });
    entries
}

/// The same, narrowed to the machines the user installed, which is what the list shows.
fn mine<'a>(catalogue: &'a Catalogue, search: &str) -> Vec<&'a Entry<PrinterProfile>> {
    sorted(catalogue, search)
        .into_iter()
        .filter(|entry| installed(&entry.source))
        .collect()
}

/// The compensation calculator over the resin form, when one is open.
fn calculators(ctx: &egui::Context, window: &mut Window) {
    let settings = &mut window.machine.settings;
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

/// The question over the window, while a deletion is waiting to be answered for.
fn ask(ctx: &egui::Context, window: &mut Window) {
    let Some(deleting) = window.machine.settings.confirm.clone() else {
        return;
    };
    let Some(answer) = confirm::ask(ctx, &window.machine.slicing.catalogue, &deleting) else {
        return;
    };
    window.machine.settings.confirm = None;
    if matches!(answer, confirm::Answer::Confirmed) {
        delete(window, deleting);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::new_printer;

    /// The shipped catalogue over a directory of its own, which every write lands in.
    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-machines-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Catalogue::with_root(&dir).expect("a missing directory reads as empty")
    }

    #[test]
    fn nothing_is_installed_until_a_machine_is_taken_out_of_the_library() {
        let mut catalogue = writable("install");
        assert!(
            mine(&catalogue, "").is_empty(),
            "a first run has no machine of its own"
        );
        assert!(
            !sorted(&catalogue, "").is_empty(),
            "the library still offers every shipped machine"
        );

        install_printer(&mut catalogue, "elegoo-mars-4-ultra").expect("the directory is writable");
        let listed: Vec<&str> = mine(&catalogue, "")
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(listed, ["elegoo-mars-4-ultra"]);

        catalogue
            .forget_user_copy(Kind::Printer, "elegoo-mars-4-ultra")
            .expect("the copy is the user's to remove");
        assert!(
            mine(&catalogue, "").is_empty(),
            "removing the copy puts the machine back in the library"
        );
    }

    /// BUG-14: the window prints with the machine the list moved to, so where the list
    /// goes has to be an answer and never the machine that was just removed.
    #[test]
    fn removing_a_machine_moves_the_list_to_the_one_under_it() {
        let mut catalogue = writable("neighbour");
        for id in [
            "elegoo-mars-3-pro",
            "elegoo-mars-4-ultra",
            "elegoo-saturn-4-ultra",
        ] {
            install_printer(&mut catalogue, id).expect("the directory is writable");
        }
        let listed: Vec<String> = mine(&catalogue, "")
            .iter()
            .map(|entry| entry.id.clone())
            .collect();

        assert_eq!(
            neighbour(&catalogue, &listed[0]).as_deref(),
            Some(listed[1].as_str()),
            "the machine under it"
        );
        assert_eq!(
            neighbour(&catalogue, &listed[2]).as_deref(),
            Some(listed[1].as_str()),
            "and the one over it when it was the last"
        );
        assert!(neighbour(&catalogue, "no-such-machine").is_none());

        for id in &listed {
            catalogue
                .forget_user_copy(Kind::Printer, id)
                .expect("the copy is the user's to remove");
        }
        assert!(
            neighbour(&catalogue, &listed[0]).is_none(),
            "nothing installed leaves the window with no machine at all"
        );
    }

    #[test]
    fn a_machine_of_your_own_stands_over_the_shipped_ones() {
        let mut catalogue = writable("own");
        let id = new_printer(&mut catalogue).expect("the directory is writable");
        let listed = sorted(&catalogue, "");
        assert_eq!(listed.first().map(|entry| entry.id.as_str()), Some(&*id));
        let brands: Vec<String> = listed[1..]
            .iter()
            .map(|entry| entry.profile.manufacturer.to_lowercase())
            .collect();
        let mut sorted_brands = brands.clone();
        sorted_brands.sort_unstable();
        assert_eq!(brands, sorted_brands, "the rest are by brand");
    }

    #[test]
    fn the_search_matches_the_brand_as_well_as_the_model() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue parses");
        let elegoo = sorted(&catalogue, "ELEGOO");
        assert!(
            elegoo
                .iter()
                .all(|entry| entry.profile.manufacturer == "Elegoo"),
            "a brand matches whatever it is typed in"
        );
        assert_eq!(sorted(&catalogue, "mars 4 ultra").len(), 1);
        assert!(sorted(&catalogue, "no such machine").is_empty());
    }
}
