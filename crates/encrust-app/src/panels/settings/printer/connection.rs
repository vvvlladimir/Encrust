//! The Network card of the printer form: what the machine speaks, and which machine on
//! the network this profile is bound to. See `docs/decisions/0156`.

use net_prusalink::Link;
use printer_profiles::Connection;

use crate::network::{Destination, Network, PrusaDraft};
use crate::ui::{Segment, Segmented, hint, icon, icon_button, theme, tone};

/// How wide a field here is: enough for a host name, not enough to stretch the card.
const FIELD_W: f32 = 230.0;

/// What this machine takes a file over, and under it the one it is sent to.
pub fn card(ui: &mut egui::Ui, profile: &str, connection: &mut Connection, network: &mut Network) {
    let segments: Vec<Segment<'_, Connection>> = Connection::ALL
        .iter()
        .map(|kind| Segment::new(*kind, kind.label()))
        .collect();
    let width = ui.available_width();
    Segmented::new(&segments).width(width).show(ui, connection);
    ui.add_space(8.0);
    match *connection {
        Connection::None => hint(
            ui,
            "What is sliced is saved to a file and carried over on a stick.",
        ),
        Connection::Sdcp => sdcp(ui, profile, network),
        Connection::PrusaLink => prusa(ui, profile, network),
    }
}

/// Boards answer a broadcast, so they are scanned for; one on another subnet is typed in.
fn sdcp(ui: &mut egui::Ui, profile: &str, network: &mut Network) {
    bound_rows(ui, profile, network, Connection::Sdcp);

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!network.is_scanning(), |ui| {
            let label = match network.is_scanning() {
                true => "Scanning...",
                false => "Scan network",
            };
            if ui.button(format!("{} {label}", icon::NETWORK)).clicked() {
                network.scan();
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut network.draft)
                .hint_text("192.168.1.42")
                .desired_width(120.0),
        );
        let address = network.draft.trim().parse().ok();
        ui.add_enabled_ui(address.is_some(), |ui| {
            if ui.button(icon::ADD).clicked()
                && let Some(address) = address
            {
                network.add(address);
                network.draft.clear();
            }
        });
    });
    hint(
        ui,
        "A board stays listed once this printer is bound to it, scan or no scan.",
    );
}

/// A Prusa machine does not answer a broadcast and wants a login, so it is typed in.
fn prusa(ui: &mut egui::Ui, profile: &str, network: &mut Network) {
    bound_rows(ui, profile, network, Connection::PrusaLink);

    let hosts: Vec<String> = network
        .prusa
        .iter()
        .map(|prusa| prusa.link.host.clone())
        .collect();
    ui.add_space(6.0);
    for host in hosts {
        if icon_button(ui, icon::REMOVE, &format!("Forget {host}")).clicked() {
            network.forget_prusa(&host);
        }
    }
    if let Some(link) = fields(ui, &mut network.prusa_draft) {
        network.add_prusa(link);
        network.prusa_draft = PrusaDraft::default();
    }
    hint(
        ui,
        "The login is kept in this window's settings, not in the profile.",
    );
}

/// The machines this protocol offers, as a choice this profile is bound to one of.
fn bound_rows(ui: &mut egui::Ui, profile: &str, network: &mut Network, speaks: Connection) {
    let mut chosen = network.bound_to(profile);
    let mut listed = false;
    for target in network.targets() {
        if !target.speaks(speaks) {
            continue;
        }
        listed = true;
        let label = format!("{} ({})", target.name(), target.detail());
        ui.radio_value(&mut chosen, Destination::Printer(target.key()), label);
    }
    if listed {
        ui.radio_value(&mut chosen, Destination::File, "Save to a file instead");
    } else {
        tone(ui, "No machine set up yet", theme::colors().text_mid);
    }
    network.bind(profile, chosen);
}

/// A machine's host and either the login its screen generated or the key it falls back
/// to. Answers with it once it is described enough to reach and add has been pressed.
fn fields(ui: &mut egui::Ui, draft: &mut PrusaDraft) -> Option<Link> {
    let half = FIELD_W / 2.0 - 2.0;
    ui.add(
        egui::TextEdit::singleline(&mut draft.host)
            .hint_text("sl1.local")
            .desired_width(FIELD_W),
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut draft.user)
                .hint_text(net_prusalink::DEFAULT_USER)
                .desired_width(half),
        );
        ui.add(
            egui::TextEdit::singleline(&mut draft.password)
                .hint_text("password")
                .password(true)
                .desired_width(half),
        );
    });
    let mut added = None;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut draft.key)
                .hint_text("or an API key")
                .password(true)
                .desired_width(FIELD_W),
        );
        let link = draft.link();
        ui.add_enabled_ui(link.is_some(), |ui| {
            if ui.button(icon::ADD).clicked() {
                added = link;
            }
        });
    });
    added
}
