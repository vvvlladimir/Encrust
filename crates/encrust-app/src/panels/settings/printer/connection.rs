//! The Network blocks of the printer form: what the machine speaks and the printers of this
//! model its files go to, each named. See `docs/decisions/0265`.

use net_prusalink::Link;
use net_sdcp::Printer;
use printer_profiles::{Connection, NetworkMachine, PrinterProfile};

use crate::i18n::tr;
use crate::network::{Network, PrusaDraft, Target, connection_label};
use crate::panels::printer_card;
use crate::panels::settings::form_card;
use crate::ui::{
    Segment, Segmented, found_row, hint, icon, icon_button, inline_button, list, subheading, theme,
    tone,
};

/// How wide a field here is: enough for a host name, not enough to stretch the card.
const FIELD_W: f32 = 230.0;
/// How wide a listed printer's name is edited.
const NAME_W: f32 = 170.0;

/// What this machine takes a file over, the printers it is sent to, and those found that
/// it could be, each a block of the form.
pub fn cards(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &mut Network) {
    form_card(ui, &tr!("connection-protocol"), |ui| {
        let segments: Vec<Segment<'_, Connection>> = Connection::ALL
            .iter()
            .map(|kind| Segment::new(*kind, connection_label(*kind)))
            .collect();
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut profile.connection);
        if profile.connection == Connection::None {
            hint(ui, &tr!("connection-usb-note"));
        }
    });
    match profile.connection {
        Connection::None => {}
        Connection::Sdcp => sdcp(ui, profile, network),
        Connection::PrusaLink => {
            form_card(ui, &tr!("connection-printers"), |ui| {
                prusa(ui, profile, network)
            });
        }
    }
}

/// Boards answer a broadcast, so the ones found are offered to add; one on another subnet
/// is typed in by its address.
fn sdcp(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &mut Network) {
    network.keep_fresh(ui.input(|input| input.time));
    form_card(ui, &tr!("connection-printers"), |ui| {
        listed(ui, profile, network);
        by_address(ui, profile, network);
    });
    form_card(ui, &tr!("connection-found"), |ui| {
        ui.horizontal(|ui| {
            let scanning = network.is_scanning();
            let label = if scanning {
                tr!("connection-scanning")
            } else {
                tr!("connection-scan-again")
            };
            if inline_button(ui, icon::RESCAN, &label, false, !scanning).clicked() {
                network.scan();
            }
        });
        found(ui, profile, network);
        printer_card::scan_notes(ui, network);
    });
}

/// A printer typed in by its address, for one a broadcast does not reach.
fn by_address(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &mut Network) {
    let address: Option<std::net::IpAddr> = network.draft.trim().parse().ok();
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = inline_button(
                ui,
                icon::ADD,
                &tr!("connection-add"),
                false,
                address.is_some(),
            );
            if add.clicked()
                && let Some(address) = address
            {
                profile.machines.push(NetworkMachine {
                    name: String::new(),
                    address: address.to_string(),
                    board: None,
                });
                network.add(address);
                network.draft.clear();
            }
            let width = ui.available_width();
            let hint = tr!("connection-address-hint");
            crate::ui::address_field(ui, &mut network.draft, &hint, width);
        });
    });
}

/// The printers this profile sends to, each named, with its state and a way to remove it. A
/// board listed by address learns its id once it answers, so a new address still finds it.
fn listed(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &mut Network) {
    if profile.machines.is_empty() {
        tone(ui, &tr!("connection-none-listed"), theme::colors().text_mid);
        return;
    }
    let speaks = profile.connection;
    let mut removed = None;
    list(ui, |ui| {
        for (index, machine) in profile.machines.iter_mut().enumerate() {
            let target = network.resolve(machine);
            if let (None, Some(Target::Sdcp(board))) = (&machine.board, &target) {
                machine.board = Some(board.mainboard_id.clone());
            }
            let low = theme::colors().text_low;
            let state = match &target {
                Some(target) if !target.speaks(speaks) => {
                    let ours = connection_label(speaks);
                    let theirs = target.protocol().label();
                    (tr!("connection-speaks", theirs = theirs, ours = ours), low)
                }
                Some(target) => {
                    printer_card::summary(network.reach(target), network.reading(target))
                }
                None if network.has_asked() => (tr!("connection-not-found"), low),
                None => (tr!("connection-not-looked"), low),
            };
            if listed_row(ui, machine, state) {
                removed = Some(index);
            }
        }
    });
    if let Some(index) = removed {
        network.forget(&profile.machines.remove(index).address);
    }
}

/// One listed printer: state dot, editable name, address, state and the button taking it
/// off the list. Answers whether that was pressed.
fn listed_row(
    ui: &mut egui::Ui,
    machine: &mut NetworkMachine,
    (state, tint): (String, egui::Color32),
) -> bool {
    let colors = theme::colors();
    let mut removed = false;
    egui::Frame::new()
        .fill(colors.raised)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, tint);
                ui.add(
                    crate::ui::text_box(egui::TextEdit::singleline(&mut machine.name))
                        .hint_text(machine.address.clone())
                        .desired_width(NAME_W),
                );
                ui.label(
                    egui::RichText::new(&machine.address)
                        .font(theme::code(11.0))
                        .color(colors.text_mid),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    removed = icon_button(ui, icon::REMOVE, &tr!("connection-take-off")).clicked();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(state).font(theme::small()).color(tint),
                        )
                        .truncate(),
                    );
                });
            });
        });
    removed
}

/// Boards that answered and are not listed yet, a box each to tick onto the list, this
/// model's first; and one press for every board of this model at once.
fn found(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &Network) {
    let mut unlisted: Vec<&Printer> = network
        .printers
        .iter()
        .filter(|board| {
            !profile.machines.iter().any(|machine| {
                let key = Target::Sdcp(board).key();
                network
                    .resolve(machine)
                    .is_some_and(|found| found.key() == key)
            })
        })
        .collect();
    unlisted.sort_by_key(|board| !same_model(&board.model, profile));
    if unlisted.is_empty() {
        let note = if network.is_scanning() {
            tr!("connection-looking")
        } else {
            tr!("connection-all-listed")
        };
        tone(ui, &note, theme::colors().text_mid);
        return;
    }
    let mut picked: Vec<Printer> = Vec::new();
    for board in &unlisted {
        let mine = same_model(&board.model, profile);
        let address = board.address.to_string();
        let response = found_row(ui, icon::PRINTER, &board.model, &address, mine);
        let response = match mine {
            true => response,
            false => response.on_hover_text(tr!("connection-other-model")),
        };
        if response.clicked() {
            picked.push((*board).clone());
        }
    }
    let same: Vec<&&Printer> = unlisted
        .iter()
        .filter(|b| same_model(&b.model, profile))
        .collect();
    if same.len() > 1
        && inline_button(
            ui,
            icon::ADD,
            &tr!("connection-add-all", count = same.len()),
            false,
            true,
        )
        .clicked()
    {
        picked = same.into_iter().map(|board| (*board).clone()).collect();
    }
    for board in picked {
        let name = default_name(&board.name, &profile.machines);
        profile.machines.push(NetworkMachine {
            name,
            address: board.address.to_string(),
            board: Some(board.mainboard_id),
        });
    }
}

/// A Prusa machine does not answer a broadcast and wants a login, so it is set up by hand
/// and the login stays in this window's settings rather than in the profile.
fn prusa(ui: &mut egui::Ui, profile: &mut PrinterProfile, network: &mut Network) {
    listed(ui, profile, network);

    let unlisted: Vec<String> = network
        .prusa
        .iter()
        .map(|prusa| prusa.link.host.clone())
        .filter(|host| {
            !profile
                .machines
                .iter()
                .any(|machine| machine.address == *host)
        })
        .collect();
    if !unlisted.is_empty() {
        subheading(ui, &tr!("connection-set-up-here"));
        for host in unlisted {
            ui.horizontal(|ui| {
                let mut ticked = false;
                if ui.checkbox(&mut ticked, &host).changed() && ticked {
                    profile.machines.push(NetworkMachine {
                        name: String::new(),
                        address: host.clone(),
                        board: None,
                    });
                }
                if icon_button(
                    ui,
                    icon::REMOVE,
                    &tr!("connection-forget-login", host = host.clone()),
                )
                .clicked()
                {
                    network.forget_prusa(&host);
                }
            });
        }
    }
    subheading(ui, &tr!("connection-set-up-another"));
    if let Some(link) = fields(ui, &mut network.prusa_draft) {
        profile.machines.push(NetworkMachine {
            name: String::new(),
            address: link.host.clone(),
            board: None,
        });
        network.add_prusa(link);
        network.prusa_draft = PrusaDraft::default();
    }
    hint(ui, &tr!("connection-login-note"));
}

/// Whether a board calling itself `model` is the machine `profile` describes.
fn same_model(model: &str, profile: &PrinterProfile) -> bool {
    let model = model.trim().to_lowercase();
    let names = [
        profile.name.to_lowercase(),
        profile.machine_name().to_lowercase(),
    ];
    !model.is_empty()
        && names
            .iter()
            .any(|name| model == *name || model.ends_with(&format!(" {name}")))
}

/// What a newly listed board is called: its own name, numbered when the list already
/// holds a printer by that name, as ten boards fresh from the box all do.
fn default_name(own: &str, listed: &[NetworkMachine]) -> String {
    let taken = |name: &str| listed.iter().any(|machine| machine.name == name);
    if !taken(own) {
        return own.to_owned();
    }
    (2..)
        .map(|n| format!("{own} {n}"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| own.to_owned())
}

/// A machine's host and either the login its screen generated or the key it falls back
/// to. Answers with it once it is described enough to reach and add has been pressed.
fn fields(ui: &mut egui::Ui, draft: &mut PrusaDraft) -> Option<Link> {
    let half = FIELD_W / 2.0 - 2.0;
    ui.add(
        crate::ui::text_box(egui::TextEdit::singleline(&mut draft.host))
            .hint_text("sl1.local")
            .desired_width(FIELD_W),
    );
    ui.horizontal(|ui| {
        ui.add(
            crate::ui::text_box(egui::TextEdit::singleline(&mut draft.user))
                .hint_text(net_prusalink::DEFAULT_USER)
                .desired_width(half),
        );
        ui.add(
            crate::ui::text_box(egui::TextEdit::singleline(&mut draft.password))
                .hint_text(tr!("connection-password"))
                .password(true)
                .desired_width(half),
        );
    });
    let mut added = None;
    ui.horizontal(|ui| {
        ui.add(
            crate::ui::text_box(egui::TextEdit::singleline(&mut draft.key))
                .hint_text(tr!("connection-api-key"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> NetworkMachine {
        NetworkMachine {
            name: name.to_owned(),
            address: "192.168.1.9".to_owned(),
            board: None,
        }
    }

    #[test]
    fn ten_boards_with_one_factory_name_are_numbered() {
        assert_eq!(default_name("3D Printer", &[]), "3D Printer");
        let listed = [named("3D Printer"), named("3D Printer 2")];
        assert_eq!(default_name("3D Printer", &listed), "3D Printer 3");
    }

    #[test]
    fn a_board_is_this_profiles_model_by_its_name_or_the_one_its_files_carry() {
        let saturn = PrinterProfile {
            name: "Saturn 4 Ultra".to_owned(),
            machine_name: Some("ELEGOO Saturn 4 Ultra".to_owned()),
            ..PrinterProfile::default()
        };
        assert!(same_model("Saturn 4 Ultra", &saturn));
        assert!(same_model("ELEGOO Saturn 4 Ultra", &saturn));
        assert!(!same_model("Saturn 3 Ultra", &saturn));
        assert!(
            !same_model("", &saturn),
            "a board that names no model is no match"
        );
    }
}
