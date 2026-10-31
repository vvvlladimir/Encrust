use egui::{Align, Layout, UiBuilder};

use crate::i18n::tr;
use crate::panels::Window;
use crate::shortcuts::{self, Action};
use crate::state::Machine;
use crate::ui::{icon, icon_button, search_button};

mod badges;
mod chip;
mod menus;
use badges::{alpha_badge, update_badge};
use chip::centred_chip;
pub use chip::layers_and_exposure;
use menus::menu;

/// The room either side of what a bar control says, and the room kept between the two
/// ends of the bar.
const PAD: f32 = 10.0;
const ENDS_GAP: f32 = 12.0;
/// A bar narrower than this shows the right end's controls as their glyphs alone.
const COMPACT_BAR_W: f32 = 720.0;

/// The bar is the window's own title bar rather than a band under one; see
/// `docs/decisions/0104` and `0217`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let bar = ui.max_rect();

    // Interacted with first so that every control drawn after it takes the press instead.
    // Not focusable: there is nothing to do with the bar from the keyboard.
    let sense = egui::Sense::CLICK | egui::Sense::DRAG;
    let drag = ui.interact(bar, ui.id().with("title-drag"), sense);
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximised(ui.ctx());
    }

    // The right end goes first, so the tabs know how far they may run.
    let compact = bar.width() < COMPACT_BAR_W;
    let right = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                window_buttons(ui);
                window_controls(ui, window);
                find_button(ui, window, compact);
                update_badge(ui, window.machine, compact);
                alpha_badge(ui, window.machine, compact);
            },
        )
        .response
        .rect;

    // The menus next, which never give way, then the chip in what is left.
    let menus = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(bar.with_max_x(right.left()))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                #[cfg(target_os = "macos")]
                ui.add_space(crate::traffic_lights::ROOM);
                menu(ui, window);
            },
        )
        .response
        .rect;
    let room = (menus.right() + ENDS_GAP)..=(right.left() - ENDS_GAP);
    centred_chip(ui, window, bar, room);
}

/// What belongs to the whole window rather than to the plate: the sheet of keys and the
/// Settings window.
fn window_controls(ui: &mut egui::Ui, window: &mut Window) {
    if icon_button(ui, icon::SETTINGS, &shortcuts::tooltip(Action::Settings)).clicked() {
        toggle_settings(window.machine);
    }
    if icon_button(ui, icon::KEYBOARD, &shortcuts::tooltip(Action::Sheet)).clicked() {
        window.view.options.sheet = true;
    }
}

fn find_button(ui: &mut egui::Ui, window: &mut Window, compact: bool) {
    let keys = shortcuts::text(Action::Find);
    let response = search_button(ui, &tr!("find-button"), &keys, compact);
    if response.on_hover_text(tr!("find-tooltip")).clicked() {
        window.view.options.find = true;
    }
}

fn toggle_maximised(ctx: &egui::Context) {
    let maximised = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximised));
}

/// macOS keeps drawing its own; every other platform has none, because the strip replaced
/// the system title bar.
#[cfg(target_os = "macos")]
fn window_buttons(_ui: &mut egui::Ui) {}

/// A browser tab is closed by the browser. In its place the page offers the source it runs,
/// at the tag it was built from, as the AGPL asks of a program served over a network.
#[cfg(target_arch = "wasm32")]
fn window_buttons(ui: &mut egui::Ui) {
    let source = concat!(
        env!("CARGO_PKG_REPOSITORY"),
        "/tree/v",
        env!("CARGO_PKG_VERSION")
    );
    ui.add_space(8.0);
    let label = egui::RichText::new(tr!("bar-source"))
        .font(theme::small())
        .color(theme::colors().text_low);
    ui.hyperlink_to(label, source)
        .on_hover_text(tr!("bar-source-tooltip"));
}

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
fn window_buttons(ui: &mut egui::Ui) {
    if crate::ui::icon_button(ui, icon::CANCEL, &tr!("bar-close")).clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let maximised = ui.input(|input| input.viewport().maximized.unwrap_or(false));
    let tooltip = if maximised {
        tr!("bar-restore")
    } else {
        tr!("bar-maximise")
    };
    if crate::ui::icon_button(ui, icon::MAXIMISE, &tooltip).clicked() {
        toggle_maximised(ui.ctx());
    }
    if crate::ui::icon_button(ui, icon::MINIMISE, &tr!("bar-minimise")).clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// Opens the Settings window, on the first support profile when none is picked yet, or
/// closes it.
pub fn toggle_settings(machine: &mut Machine) {
    let settings = &mut machine.settings;
    if settings.open {
        settings.close();
        return;
    }
    settings.open = true;
    let catalogue = &machine.slicing.catalogue;
    if settings.support.is_none()
        && let Some(first) = catalogue.supports().next()
    {
        let id = first.id.clone();
        settings.pick_support(catalogue, &id);
    }
}

/// Opens the Machine and resin window on the machine the plate is printed on.
pub fn open_machines(machine: &mut Machine) {
    let printer = machine.slicing.printer_id.clone();
    machine
        .settings
        .open_machines(&machine.slicing.catalogue, printer.as_deref(), None);
}

#[cfg(test)]
mod tests {
    use super::chip::share_room;

    #[test]
    fn names_with_room_are_not_cut() {
        assert_eq!(share_room(300.0, 120.0, 90.0), (120.0, 90.0));
    }

    #[test]
    fn the_longer_name_is_cut_first() {
        assert_eq!(share_room(200.0, 160.0, 60.0), (140.0, 60.0));
        assert_eq!(share_room(200.0, 60.0, 160.0), (60.0, 140.0));
    }

    #[test]
    fn two_long_names_share_the_room_evenly() {
        assert_eq!(share_room(200.0, 160.0, 180.0), (100.0, 100.0));
    }

    #[test]
    fn no_room_cuts_both_names_to_nothing() {
        assert_eq!(share_room(-40.0, 160.0, 60.0), (0.0, 0.0));
    }
}
