//! The save step: the plate sliced into a file as the window's Slice to slices it, named
//! by a template of ADR 0387's tags, and sent on as the plate card's send sends it.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use core_recipe::save_folder;
use printer_profiles::Connection;

use super::step::Ended;
use super::work::Saved;
use crate::i18n::tr;
use crate::scene::Scene;
use crate::state::Machine;
use crate::status::Status;

/// A file a run writes: where, whether it goes on to the printer, and whether its slicing
/// has started.
#[derive(Debug)]
pub(super) struct Saving {
    path: PathBuf,
    send: bool,
    started: bool,
}

impl Saving {
    /// Where `saved` writes the plate, or why it cannot send it on.
    pub(super) fn new(saved: &Saved, scene: &Scene, machine: &Machine) -> Result<Self> {
        if saved.send
            && let Some(why) = send_blocker(machine)
        {
            bail!(why);
        }
        let general = &machine.general;
        let template = saved.name.as_deref().unwrap_or(&general.file_name);
        let name = crate::panels::file_name(scene, &machine.slicing, template);
        let folder = folder_of(saved.folder.as_deref(), general.sliced_dir.as_deref());
        Ok(Self {
            path: path_in(folder, &name),
            send: saved.send,
            started: false,
        })
    }

    /// Starts slicing once nothing else slices and the supports are built, and lands once
    /// the file is written; the window sends it on from there.
    pub(super) fn poll(&mut self, scene: &Scene, machine: &mut Machine) -> Option<Ended> {
        let slicing = &mut machine.slicing;
        if self.started {
            if slicing.job.is_some() {
                return None;
            }
            let wrote = slicing.wrote() == Some(self.path.as_path());
            let said = machine.status.clone();
            return Some(if wrote {
                Ended::Landed(said)
            } else {
                Ended::Stopped(said)
            });
        }
        if slicing.job.is_some() || scene.supports_pending(scene.active_plate()) {
            return None;
        }
        if let Some(why) = slicing.blocker(scene) {
            return Some(Ended::Stopped(Status::Error(why)));
        }
        let path = self.path.display().to_string();
        let started = slicing.start(scene, scene.active_plate(), self.path.clone());
        if let Err(error) =
            started.with_context(|| tr!("slice-cannot-slice-into", path = path.clone()))
        {
            return Some(Ended::Stopped(Status::failed(&error)));
        }
        if self.send {
            slicing.send_when_written();
        }
        self.started = true;
        machine.status = Status::Info(tr!("slice-slicing-into", path = path));
        None
    }
}

/// Why the file cannot go on to the printer in hand: a browser reaches none, a profile may
/// name none, or the one it names is not found, cannot take the file or is busy.
fn send_blocker(machine: &Machine) -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        return Some(tr!("printer-browser-sends-nothing"));
    }
    let connection = machine
        .slicing
        .printer
        .as_ref()
        .map_or(Connection::None, |profile| profile.connection);
    let network = &machine.network;
    let target = network.target();
    if connection == Connection::None || !target.is_some_and(|target| target.speaks(connection)) {
        return Some(tr!("prepare-save-no-printer"));
    }
    network
        .blocker(machine.slicing.format)
        .or_else(|| network.busy())
}

/// The folder a step names, `~` its user's home; else where the window saves sliced files.
fn folder_of(asked: Option<&str>, sliced_dir: Option<&Path>) -> PathBuf {
    let home = std::env::home_dir();
    match asked {
        Some(folder) => save_folder(folder, home.as_deref()),
        None => sliced_dir
            .map(Path::to_path_buf)
            .or(home)
            .unwrap_or_default(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn path_in(folder: PathBuf, name: &str) -> PathBuf {
    folder.join(name)
}

/// A browser downloads the file, under its name alone.
#[cfg(target_arch = "wasm32")]
fn path_in(_folder: PathBuf, name: &str) -> PathBuf {
    PathBuf::from(name)
}
