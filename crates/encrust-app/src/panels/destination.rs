use printer_profiles::Connection;

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::printer_card::{self, Chosen};
use crate::state::Machine;
use crate::ui::theme;

/// The printer this plate's file goes to, as a row of the plate's card over Slice: its name
/// and state, the others the profile lists, a new scan and its sheet (ADR 0348).
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let connection = connection(window.machine);
    if connection == Connection::None || window.machine.network.listed().is_empty() {
        return;
    }
    window
        .machine
        .network
        .keep_fresh(ui.input(|input| input.time));
    let network = &window.machine.network;
    let warning = network
        .target()
        .filter(|target| !target.speaks(connection))
        .map(|target| {
            tr!(
                "slice-speaks-other",
                name = network.label(&target),
                theirs = target.protocol().label(),
                ours = crate::network::connection_label(connection)
            )
        });
    let chosen = printer_card::destination(ui, network, warning);
    ui.add_space(theme::ITEM_GAP);
    match chosen {
        Some(Chosen::Pick(address)) => window.machine.network.choose(address),
        Some(Chosen::Open(key)) => window.machine.network.open = Some(key),
        Some(Chosen::Scan) => window.machine.network.scan(),
        None => {}
    }
}

/// How the printer in hand takes its files; a browser reaches no printer (ADR 0182).
pub(super) fn connection(machine: &Machine) -> Connection {
    if cfg!(target_arch = "wasm32") {
        return Connection::None;
    }
    machine
        .slicing
        .printer
        .as_ref()
        .map_or(Connection::None, |profile| profile.connection)
}
