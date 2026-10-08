//! What the window remembers between runs: the machine, the resin, the values every tool
//! is set to, the printer on the network each machine sends to, and whether to look for
//! updates.

use std::collections::BTreeMap;
use std::net::IpAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use net_prusalink::Link;
use net_sdcp::Printer;

use crate::network::{Destination, Network, Prusa};
use crate::panels::Window;
use crate::profiles;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;
use crate::updates::UpdatePrefs;

/// The choices carried from one run to the next, beside the user's profile directory.
///
/// Only catalogue ids are kept. A profile opened from a file is not remembered: the file
/// may have moved, and a silently stale machine is worse than none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// What every tool was last set to, or `None` for a file written before they were
    /// remembered. A value the user chose once is theirs until they change it again; see
    /// `docs/decisions/0192`.
    #[serde(default)]
    tools: Option<ToolSettings>,
}

impl Preferences {
    /// What the window is set to now.
    pub fn of(slicing: &Slicing, network: &Network, updates: &UpdatePrefs, tools: &Tools) -> Self {
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
            tools: Some(ToolSettings::of(tools, slicing)),
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
        // After the resin, which brings a layer height and exposures of its own: what the
        // user last cut at stands over what the profile was measured at.
        if let Some(tools) = self.tools.clone() {
            tools.apply(window.tools, &mut window.machine.slicing);
        }
    }

    /// Writes the file, making its directory first. A failure is silent: nothing the user
    /// asked for has failed, and the window has no room to say it.
    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            store(&text);
        }
    }
}

/// What was remembered, or nothing at all when the file is missing or unreadable.
pub fn load() -> Preferences {
    let Some(text) = stored() else {
        return Preferences::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn store(text: &str) {
    let Some(path) = file() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn stored() -> Option<String> {
    file().and_then(|path| std::fs::read_to_string(path).ok())
}

/// The page's own storage, where a browser keeps what a desktop keeps in a file.
#[cfg(target_arch = "wasm32")]
const KEY: &str = "encrust.preferences";

#[cfg(target_arch = "wasm32")]
fn store(text: &str) {
    crate::web::remember(KEY, text);
}

#[cfg(target_arch = "wasm32")]
fn stored() -> Option<String> {
    crate::web::remembered(KEY)
}

/// Beside the user's profile directory, because both are this application's own settings
/// and a user moving one expects the other to follow.
#[cfg(not(target_arch = "wasm32"))]
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
        let prefs = Preferences::of(
            &slicing,
            &Network::default(),
            &UpdatePrefs::default(),
            &Tools::default(),
        );
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

        let prefs = Preferences::of(
            &Slicing::default(),
            &network,
            &UpdatePrefs::default(),
            &Tools::default(),
        );
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
        let prefs = Preferences::of(
            &Slicing::default(),
            &network,
            &UpdatePrefs::default(),
            &Tools::default(),
        );
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
        let prefs = Preferences::of(
            &Slicing::default(),
            &network,
            &UpdatePrefs::default(),
            &Tools::default(),
        );
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
            read.updates.check,
            "the update check is on in a file written before it existed"
        );
    }

    /// A wall thickness typed in once is the user's from then on, so it travels through
    /// the file beside the machine it was typed for; see ADR 0192.
    #[test]
    fn the_values_the_tools_were_left_at_survive_the_round_trip() {
        let mut tools = Tools::default();
        tools.hollow.state.thickness_mm = 1.25;
        tools.drain.state.depth_mm = 7.5;
        tools.supports.brush_radius_mm = 4.0;

        let prefs = Preferences::of(
            &Slicing::default(),
            &Network::default(),
            &UpdatePrefs::default(),
            &tools,
        );
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");

        let mut back = Tools::default();
        let mut slicing = Slicing::default();
        read.tools
            .clone()
            .expect("the tool values were written")
            .apply(&mut back, &mut slicing);
        assert_eq!(back.hollow.state.thickness_mm, 1.25);
        assert_eq!(back.drain.state.depth_mm, 7.5);
        assert_eq!(back.supports.brush_radius_mm, 4.0);
    }

    #[test]
    fn the_update_check_survives_the_round_trip() {
        let updates = UpdatePrefs {
            check: true,
            checked_at_s: Some(1_700_000_000),
            skipped: Some("0.3.0".to_owned()),
        };
        let prefs = Preferences::of(
            &Slicing::default(),
            &Network::default(),
            &updates,
            &Tools::default(),
        );
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read.updates, updates);
    }
}
