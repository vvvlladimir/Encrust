//! What the window remembers between runs: the machine, the resin, the values every tool
//! is set to, the printer on the network each machine sends to, whether to look for
//! updates, how the window behaves, how wide the two side cards were dragged, where
//! Preview divides the model from the mask, and what each panel was switched to.

use std::collections::BTreeMap;
use std::net::IpAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use net_prusalink::Link;
use net_sdcp::Printer;
use printer_profiles::{Kind, Renamed};

use crate::drain;
use crate::gizmo::Axes;
use crate::i18n::Language;
use crate::measure::{Snap, Target};
use crate::network::{Network, Prusa};
use crate::panels::Window;
use crate::profiles;
use crate::recent::Recent;
use crate::relief::ReliefChoice;
use crate::repair::{Picking, RepairMode};
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::status::Status;
use crate::supports;
use crate::tool_settings::ToolSettings;
use crate::ui::kept::Kept;
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
    #[serde(default)]
    relief: ReliefChoice,
    /// The files opened lately, newest first.
    #[serde(default)]
    recent: Recent,
    /// Where Preview divides the model from the mask, or `None` for the default.
    #[serde(default)]
    mask_split: Option<f32>,
    /// How wide the inspector and the plate panel were dragged, points, or `None` for the
    /// defaults; `inspector_w` is new so a column dragged before ADR 0382 is forgotten.
    #[serde(default)]
    inspector_w: Option<f32>,
    #[serde(default)]
    plate_w: Option<f32>,
    #[serde(default)]
    general: General,
    #[serde(default)]
    modes: Modes,
    /// Which sections were folded and which tab of a panel was open, by widget.
    #[serde(default)]
    kept: Kept,
}

/// What each tool's panel was switched to that is neither a value of the tool nor undone,
/// like a view: what a click puts down, what Measure catches, which axes the handles
/// follow. See `docs/decisions/0334`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Modes {
    supports: supports::Placing,
    drain: drain::Placing,
    snap: Snap,
    target: Target,
    show_axes: bool,
    axes: Axes,
    repair: RepairMode,
    picking: Picking,
}

impl Modes {
    /// What the panels are switched to now.
    pub fn of(tools: &Tools, axes: Axes) -> Self {
        Self {
            supports: tools.supports.placing,
            drain: tools.drain.placing,
            snap: tools.measure.snap,
            target: tools.measure.target,
            show_axes: tools.measure.show_axes,
            axes,
            repair: tools.repair.mode,
            picking: tools.repair.picking,
        }
    }

    fn apply(&self, tools: &mut Tools, axes: &mut Axes) {
        tools.supports.placing = self.supports;
        tools.drain.placing = self.drain;
        tools.measure.snap = self.snap;
        tools.measure.target = self.target;
        tools.measure.show_axes = self.show_axes;
        *axes = self.axes;
        tools.repair.mode = self.repair;
        tools.repair.picking = self.picking;
    }
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
    /// Whether nothing in the window animates.
    pub reduce_motion: bool,
    /// Whether the window is light, dark, or follows the system.
    pub appearance: Appearance,
}

/// The palette the user asked for. `System` follows the platform's own setting as it
/// changes; see `docs/decisions/0383-a-light-palette-chosen-each-frame.md`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// What egui is told, so it resolves the theme each frame.
    pub fn preference(self) -> egui::ThemePreference {
        match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_page: true,
            invert_zoom: false,
            sliced_dir: None,
            language: None,
            reduce_motion: false,
            appearance: Appearance::System,
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
            relief: tools.relief.choice(),
            recent: Recent::default(),
            mask_split: None,
            inspector_w: None,
            plate_w: None,
            general: General::default(),
            modes: Modes::of(tools, Axes::default()),
            kept: Kept::default(),
        }
    }

    /// The same, with which axes the handles follow and what the panels themselves chose.
    pub fn with_panels(self, axes: Axes, ctx: &egui::Context) -> Self {
        Self {
            modes: Modes { axes, ..self.modes },
            kept: Kept::of(ctx),
            ..self
        }
    }

    /// Puts back what the panels themselves chose, where their widgets look for it.
    pub fn restore_panels(&self, ctx: &egui::Context) {
        self.kept.clone().restore(ctx);
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
            inspector_w: options.inspector_w,
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
                Kind::Support | Kind::Hollow | Kind::Drain | Kind::Auto => continue,
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
            (self.plate_w, &mut options.plate_w, theme::SCENE_W_RANGE),
        ] {
            if let Some(stored) = stored.filter(|stored| stored.is_finite()) {
                *value = stored.clamp(*range.start(), *range.end());
            }
        }
        let range = theme::INSPECTOR_W_RANGE;
        options.inspector_w = self
            .inspector_w
            .filter(|stored| stored.is_finite())
            .map(|stored| stored.clamp(*range.start(), *range.end()));
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
        self.modes.apply(window.tools, &mut window.view.gizmo.axes);
        if let Err(error) = window.tools.relief.restore(&self.relief) {
            window.machine.status = Status::failed(&error);
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
mod tests;
