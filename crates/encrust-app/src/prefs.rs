//! What the window remembers between runs: the machine, the resin, the values every tool
//! is set to, the printer on the network each machine sends to, whether to look for
//! updates, how the window behaves, how wide the two side cards were dragged, and where
//! Preview divides the model from the mask.

use std::collections::BTreeMap;
use std::net::IpAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use net_prusalink::Link;
use net_sdcp::Printer;
use printer_profiles::{Kind, Renamed};

use crate::i18n::Language;
use crate::network::{Network, Prusa};
use crate::panels::Window;
use crate::profiles;
use crate::recent::Recent;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;
use crate::ui::theme;
use crate::updates::UpdatePrefs;
use crate::workspace::ViewOptions;

/// The choices carried from one run to the next, beside the user's profile directory.
///
/// Only catalogue ids are kept, never a path: a profile handed over as a file is imported
/// into the catalogue first, because the file may move and a stale machine is worse than none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    printer: Option<String>,
    resin: Option<String>,
    /// Printer addresses the user typed. A printer found by broadcast is not kept: it
    /// announces itself again next run, and its address may not be the same one.
    #[serde(default)]
    addresses: Vec<IpAddr>,
    /// Which of its listed printers each profile last sent to, by catalogue id, as the
    /// printer's address; see ADR 0265.
    #[serde(default)]
    chosen: BTreeMap<String, String>,
    /// The boards the printer in hand lists, so they are named before any scan answers.
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
    /// The files opened lately, newest first.
    #[serde(default)]
    recent: Recent,
    /// Where Preview divides the model from the mask, or `None` for the default.
    #[serde(default)]
    mask_split: Option<f32>,
    /// How wide the inspector and the plate panel were dragged, points, or `None` for the
    /// defaults.
    #[serde(default)]
    inspector_w: Option<f32>,
    #[serde(default)]
    plate_w: Option<f32>,
    #[serde(default)]
    general: General,
}

/// How the window itself behaves, as the Settings window's General page sets it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Whether a run opens on the start page rather than straight on the plate.
    pub start_page: bool,
    /// Whether scrolling down zooms in rather than out.
    pub invert_zoom: bool,
    /// Where the dialog a sliced file is saved from opens, or `None` for the system's choice.
    pub sliced_dir: Option<std::path::PathBuf>,
    /// The language the window is written in, or `None` for the system's.
    pub language: Option<Language>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_page: true,
            invert_zoom: false,
            sliced_dir: None,
            language: None,
        }
    }
}

impl Preferences {
    /// What the window is set to now.
    pub fn of(slicing: &Slicing, network: &Network, updates: &UpdatePrefs, tools: &Tools) -> Self {
        Self {
            printer: slicing.printer_id.clone(),
            resin: slicing.resin_id.clone(),
            addresses: network.manual.clone(),
            chosen: network.chosen().clone(),
            boards: network.bound_boards(),
            machines: network
                .prusa
                .iter()
                .map(|prusa| prusa.link.clone())
                .collect(),
            updates: updates.clone(),
            tools: Some(ToolSettings::of(tools, slicing)),
            recent: Recent::default(),
            mask_split: None,
            inspector_w: None,
            plate_w: None,
            general: General::default(),
        }
    }

    /// The same, with how the window behaves.
    pub fn with_general(self, general: &General) -> Self {
        Self {
            general: general.clone(),
            ..self
        }
    }

    /// The same, with the files opened lately.
    pub fn with_recent(self, recent: &Recent) -> Self {
        Self {
            recent: recent.clone(),
            ..self
        }
    }

    /// The same, with where Preview divides the model from the mask and how wide the side
    /// cards stand.
    pub fn with_layout(self, options: &ViewOptions) -> Self {
        Self {
            mask_split: Some(options.mask_split),
            inspector_w: Some(options.inspector_w),
            plate_w: Some(options.plate_w),
            ..self
        }
    }

    /// The language the window is written in, or `None` for the system's.
    pub fn language(&self) -> Option<Language> {
        self.general.language
    }

    /// Points every id kept here at where the catalogue moved its profile.
    pub fn follow(&mut self, renamed: &[Renamed]) {
        for moved in renamed {
            let held = match moved.kind {
                Kind::Printer => &mut self.printer,
                Kind::Resin => &mut self.resin,
                Kind::Support | Kind::Hollow | Kind::Drain => continue,
            };
            if held.as_deref() == Some(moved.from.as_str()) {
                *held = Some(moved.to.clone());
            }
            if moved.kind == Kind::Printer
                && let Some(address) = self.chosen.remove(&moved.from)
            {
                self.chosen.insert(moved.to.clone(), address);
            }
        }
    }

    /// Stands the window under what was remembered. An id no longer in the catalogue is
    /// dropped rather than reported: the user did not ask for it this run.
    pub fn apply(&self, window: &mut Window) {
        window.machine.updates.prefs.clone_from(&self.updates);
        window.machine.recent.clone_from(&self.recent);
        window.machine.general.clone_from(&self.general);
        let options = &mut window.view.options;
        if !self.general.start_page {
            options.started = true;
        }
        for (stored, value, range) in [
            (
                self.mask_split,
                &mut options.mask_split,
                theme::MASK_SPLIT_RANGE,
            ),
            (
                self.inspector_w,
                &mut options.inspector_w,
                theme::INSPECTOR_W_RANGE,
            ),
            (self.plate_w, &mut options.plate_w, theme::SCENE_W_RANGE),
        ] {
            if let Some(stored) = stored.filter(|stored| stored.is_finite()) {
                *value = stored.clamp(*range.start(), *range.end());
            }
        }
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
            .restore(self.boards.clone(), self.chosen.clone());

        // Only what is still the user's: a machine they removed is still in the library.
        if let Some(id) = self.printer.as_deref()
            && let Ok(entry) = window.machine.slicing.catalogue.printer(id)
            && crate::settings::installed(&entry.source)
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
            // A resin not set up on that machine gives way to one that is, or to none.
            window.machine.slicing.reload_resin();
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
    use crate::scene::Scene;
    use printer_profiles::Raft;

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

    /// The printer a profile last sent to, and the board it lists, travel through the
    /// file so the next run sends there before any scan has answered.
    #[test]
    fn the_printer_a_profile_last_sent_to_survives_the_round_trip() {
        let mut network = Network::default();
        let board = a_board();
        let address = board.address.to_string();
        network.restore(vec![board], BTreeMap::new());
        let profile = printer_profiles::PrinterProfile {
            connection: printer_profiles::Connection::Sdcp,
            machines: vec![printer_profiles::NetworkMachine {
                name: "Bench".to_owned(),
                address: address.clone(),
                board: None,
            }],
            ..printer_profiles::PrinterProfile::default()
        };
        network.bind_to(Some("elegoo-saturn-4-ultra".to_owned()), Some(&profile));
        network.choose(address.clone());

        let prefs = Preferences::of(
            &Slicing::default(),
            &network,
            &UpdatePrefs::default(),
            &Tools::default(),
        );
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");

        assert_eq!(read.boards.len(), 1, "the listed board is kept");
        assert_eq!(read.chosen.get("elegoo-saturn-4-ultra"), Some(&address));
    }

    #[test]
    fn a_profile_moved_to_its_own_id_is_followed() {
        let mut prefs = Preferences {
            printer: Some("bench".to_owned()),
            resin: Some("grey".to_owned()),
            chosen: BTreeMap::from([("bench".to_owned(), "192.168.1.42".to_owned())]),
            ..Preferences::default()
        };
        let moved = |kind, from: &str, to: &str| Renamed {
            kind,
            from: from.to_owned(),
            to: to.to_owned(),
        };

        prefs.follow(&[
            moved(Kind::Printer, "bench", "bench-0a1b2c3d"),
            moved(Kind::Resin, "grey", "grey-4e5f6a7b"),
        ]);

        assert_eq!(prefs.printer.as_deref(), Some("bench-0a1b2c3d"));
        assert_eq!(prefs.resin.as_deref(), Some("grey-4e5f6a7b"));
        assert!(
            prefs.chosen.contains_key("bench-0a1b2c3d"),
            "the printer it sends to follows"
        );
        assert!(!prefs.chosen.contains_key("bench"));
    }

    /// Only the boards a profile lists: one that merely answered a scan announces
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
        assert!(read.chosen.is_empty());
        assert!(
            read.updates.check,
            "the update check is on in a file written before it existed"
        );
    }

    #[test]
    fn the_preview_divide_and_the_card_widths_survive_the_round_trip() {
        let options = ViewOptions {
            mask_split: 0.62,
            inspector_w: 400.0,
            plate_w: 200.0,
            ..ViewOptions::default()
        };
        let prefs = Preferences::default().with_layout(&options);
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read.mask_split, Some(0.62));
        assert_eq!(read.inspector_w, Some(400.0));
        assert_eq!(read.plate_w, Some(200.0));
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
    fn the_plates_raft_is_the_next_runs() {
        let mut tools = Tools::default();
        let plate = Raft {
            enabled: !tools.supports.raft.enabled,
            ..tools.supports.raft
        };
        tools.supports.keep_raft(plate, &mut Scene::default());
        let prefs = Preferences::of(
            &Slicing::default(),
            &Network::default(),
            &UpdatePrefs::default(),
            &tools,
        );
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let back: Preferences = serde_json::from_str(&text).expect("and read back");

        let mut next = Tools::default();
        back.tools
            .expect("the tool values were written")
            .apply(&mut next, &mut Slicing::default());
        assert_eq!(
            next.supports.raft, plate,
            "the switch stays where it was left"
        );
    }

    #[test]
    fn a_file_written_before_the_general_page_opens_on_the_start_page() {
        let read: Preferences =
            serde_json::from_str("{}").expect("every field of the file is optional");
        assert!(read.general.start_page);
        assert!(!read.general.invert_zoom);
    }

    #[test]
    fn how_the_window_behaves_survives_the_round_trip() {
        let general = General {
            start_page: false,
            invert_zoom: true,
            sliced_dir: Some(std::path::PathBuf::from("/prints")),
            language: Some(crate::i18n::Language::Russian),
        };
        let prefs = Preferences::default().with_general(&general);
        let text = serde_json::to_string(&prefs).expect("the preferences serialise");
        let read: Preferences = serde_json::from_str(&text).expect("and read back");
        assert_eq!(read.general, general);
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
