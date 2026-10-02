//! What the window remembers between runs: the machine, the resin, the printer on the
//! network each machine sends to, and whether to look for updates.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use net_prusalink::Link;
use net_sdcp::Printer;

use crate::network::{Destination, Network, Prusa};
use crate::panels::Window;
use crate::profiles;
use crate::slicing::Slicing;
use crate::updates::UpdatePrefs;

/// The choices carried from one run to the next, beside the user's profile directory.
///
/// Only catalogue ids are kept. A profile opened from a file is not remembered: the file
/// may have moved, and a silently stale machine is worse than none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    printer: Option<String>,
    resin: Option<String>,
    /// Printer addresses the user typed. A printer found by broadcast is not kept: it
    /// announces itself again next run, and its address may not be the same one.
    #[serde(default)]
    addresses: Vec<IpAddr>,
    /// Where each printer profile sends what it slices, by catalogue id; see ADR 0156.
    #[serde(default)]
    bound: BTreeMap<String, Destination>,
    /// The boards those bindings name, so a bound machine is there to send to before any
    /// scan has answered.
    #[serde(default)]
    boards: Vec<Printer>,
    /// The Prusa machines set up, credentials and all. They are here rather than in a
    /// printer profile because they are this window's settings and not the machine's;
    /// see `docs/decisions/0153-a-prusa-machine-is-set-up-not-discovered.md`.
    #[serde(default)]
    machines: Vec<Link>,
    #[serde(default)]
    updates: UpdatePrefs,
}

impl Preferences {
    /// What the window is set to now.
    pub fn of(slicing: &Slicing, network: &Network, updates: &UpdatePrefs) -> Self {
        Self {
            printer: slicing.printer_id.clone(),
            resin: slicing.resin_id.clone(),
            addresses: network.manual.clone(),
            bound: network.bindings().clone(),
            boards: network.bound_boards(),
            machines: network
                .prusa
                .iter()
                .map(|prusa| prusa.link.clone())
                .collect(),
            updates: updates.clone(),
        }
    }

    /// Stands the window under what was remembered. An id no longer in the catalogue is
    /// dropped rather than reported: the user did not ask for it this run.
    pub fn apply(&self, window: &mut Window) {
        window.machine.updates.prefs.clone_from(&self.updates);
        // Before the printer, so that applying it points the window at the machine this
        // profile was last sent to.
        window.machine.network.manual.clone_from(&self.addresses);
        window.machine.network.prusa = self
            .machines
            .iter()
            .map(|link| Prusa {
                link: link.clone(),
                text: None,
            })
            .collect();
        window
            .machine
            .network
            .restore(self.boards.clone(), self.bound.clone());

        if let Some(id) = self.printer.as_deref()
            && let Ok(entry) = window.machine.slicing.catalogue.printer(id)
        {
            let profile = entry.profile.clone();
            profiles::apply_printer(window, profile, Some(id.to_owned()));
        }
        if let Some(id) = self.resin.as_deref()
            && let Ok(entry) = window.machine.slicing.catalogue.resin(id)
        {
            let resin = entry.profile.clone();
            window.machine.slicing.resin_id = Some(id.to_owned());
            window.machine.slicing.set_material(resin);
        }
    }

    /// Writes the file, making its directory first. A failure is silent: nothing the user
    /// asked for has failed, and the window has no room to say it.
    pub fn save(&self) {
        let Some(path) = file() else {
            return;
        };
        let Some(dir) = path.parent() else {
            return;
        };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// What was remembered, or nothing at all when the file is missing or unreadable.
pub fn load() -> Preferences {
    let Some(text) = file().and_then(|path| std::fs::read_to_string(path).ok()) else {
        return Preferences::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Beside the user's profile directory, because both are this application's own settings
/// and a user moving one expects the other to follow.
fn file() -> Option<PathBuf> {
    let dir = printer_profiles::user_dir()?;
    Some(dir.with_file_name("preferences.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_remembered_is_what_the_window_is_set_to() {
        let mut slicing = Slicing::default();
        slicing.resin_id = Some("standard-grey".to_owned());
        let prefs = Preferences::of(&slicing, &Network::default(), &UpdatePrefs::default());
        assert_eq!(prefs.resin.as_deref(), Some("standard-grey"));
    }

    /// Clicking through a scan once a run is what the binding is there to avoid, so the
    /// board travels through the file beside the profile it was chosen for.
    #[test]
    fn the_machine_a_printer_was_bound_to_survives_the_round_trip() {
        let mut network = Network::default();
        let board = a_board();
        let key = format!("sdcp:{}", board.mainboard_id);
        network.restore(vec![board], BTreeMap::new());
        network.bind("elegoo-saturn-4-ultra", Destination::Printer(key.clone()));

        let prefs = Preferences::of(&Slicing::default(), &network, &UpdatePrefs::default());
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");

        assert_eq!(read.boards.len(), 1, "the bound board is kept");
        assert_eq!(
            read.bound.get("elegoo-saturn-4-ultra"),
            Some(&Destination::Printer(key))
        );
    }

    /// Only the boards a profile was bound to: one that merely answered a scan announces
    /// itself again next run, at whatever address it has then.
    #[test]
    fn a_board_nobody_chose_is_not_written_to_the_file() {
        let mut network = Network::default();
        network.restore(vec![a_board()], BTreeMap::new());
        let prefs = Preferences::of(&Slicing::default(), &network, &UpdatePrefs::default());
        assert!(prefs.boards.is_empty());
    }

    fn a_board() -> Printer {
        Printer {
            name: "Saturn".to_owned(),
            model: "Saturn 4 Ultra".to_owned(),
            brand: "ELEGOO".to_owned(),
            address: IpAddr::from([192, 168, 1, 42]),
            mainboard_id: "aa".to_owned(),
            brand_id: "00".to_owned(),
            firmware: String::new(),
            protocol: String::new(),
            transport: net_sdcp::Transport::WebSocket,
        }
    }

    /// A machine that has to be typed in again every run is a machine nobody sends to, so
    /// the credentials travel through the file with it.
    #[test]
    fn a_prusa_machine_survives_the_round_trip_through_the_file() {
        let mut network = Network::default();
        network.add_prusa(Link::digest(
            "sl1.local",
            net_prusalink::DEFAULT_USER,
            "secret",
        ));
        let prefs = Preferences::of(&Slicing::default(), &network, &UpdatePrefs::default());
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read.machines, prefs.machines);
        assert_eq!(
            read.machines.first().map(|link| link.host.as_str()),
            Some("sl1.local")
        );
    }

    #[test]
    fn a_file_written_before_machines_existed_still_reads() {
        let read: Preferences = serde_json::from_str(r#"{"format":".goo"}"#)
            .expect("every field of the file is optional");
        assert!(read.machines.is_empty());
        assert!(read.bound.is_empty());
        assert!(
            !read.updates.check,
            "the update check stays off until turned on"
        );
    }

    #[test]
    fn the_update_check_survives_the_round_trip() {
        let updates = UpdatePrefs {
            check: true,
            checked_at_s: Some(1_700_000_000),
            skipped: Some("0.3.0".to_owned()),
        };
        let prefs = Preferences::of(&Slicing::default(), &Network::default(), &updates);
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read.updates, updates);
    }
}
