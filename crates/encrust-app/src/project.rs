//! The `.encrust` project file as the window handles it: the dialogs, and whether the
//! plate in hand matches the file it came from.
//!
//! The format itself — the manifest, the mesh blobs and their (de)serialisation — is
//! `core_engine::project`. What the window converts to and from it is `state`.

pub mod state;

use anyhow::Context as _;
use core_engine::project::{Cavity, EXTENSION, Project, digest, load, save};

use crate::panels::Window;
use crate::panels::frame_view;
use crate::scene::Scene;
use crate::status::Status;
use crate::undo::History;

/// The project file the window has open, so that Save knows where to write and the title
/// strip knows what to call the plate.
#[derive(Debug, Default)]
pub struct Opened {
    pub path: Option<std::path::PathBuf>,
    /// The digest of the plate as it was last written or read, or `None` for a plate that
    /// has never been either. Unsaved work is this not matching the plate in hand.
    saved: Option<u64>,
    /// The window is trying to close and is waiting to be told what to do about the work
    /// that is not saved.
    asking: bool,
    /// The answer was to close anyway, so the next close request is not questioned again.
    closing: bool,
}

impl Opened {
    /// What the title strip calls the plate: the file's own name, or nothing at all for a
    /// plate that has never been saved.
    pub fn name(&self) -> Option<&str> {
        self.path.as_ref()?.file_stem()?.to_str()
    }
}

/// Takes everything off the plate and forgets which file it came from. The chosen
/// profiles and the tools' own numbers stay: they are how this user works, not what is
/// on the plate.
pub fn clear(window: &mut Window) {
    window.doc.scene = Scene::default();
    window.doc.history = History::default();
    window.doc.project.path = None;
    window.doc.project.saved = Some(digest(&captured(window).manifest));
    window.machine.status = Status::Info("New plate".to_owned());
}

/// Opens a project over whatever the window was holding.
pub fn open_dialog(window: &mut Window) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Encrust project", &[EXTENSION])
        .pick_file()
    else {
        return;
    };
    let read = load(&path)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot open the project {}", path.display()));
    let Some(project) = window
        .machine
        .status
        .report(&format!("Opened {}", path.display()), read)
    else {
        return;
    };

    let cavities: Vec<Option<Cavity>> = project
        .manifest
        .objects
        .iter()
        .map(|object| object.hollow.cavity)
        .collect();
    state::apply(
        project,
        state::CapturedMut {
            scene: &mut window.doc.scene,
            slicing: &mut window.machine.slicing,
            supports: &mut window.tools.supports,
            hollow: &mut window.tools.hollow,
            drain: &mut window.tools.drain,
            cut: &mut window.tools.cut,
            array: &mut window.tools.array,
        },
    );
    window.doc.project.path = Some(path);
    // The cavities are still being built on a worker; the plate counts as saved in the
    // state it reaches once they are.
    let mut manifest = captured(window).manifest;
    for (object, cavity) in manifest.objects.iter_mut().zip(cavities) {
        object.hollow.cavity = cavity;
    }
    window.doc.project.saved = Some(digest(&manifest));
    window.doc.history = History::default();
    frame_view(
        &window.doc.scene,
        &window.doc.plate,
        &mut window.view.camera,
    );
}

/// Writes over the file the plate came from, or asks for a name when it came from none.
pub fn save_open(window: &mut Window) {
    match window.doc.project.path.clone() {
        Some(path) => write(window, path),
        None => save_dialog(window),
    }
}

pub fn save_dialog(window: &mut Window) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Encrust project", &[EXTENSION])
        .set_file_name(format!("plate.{EXTENSION}"))
        .save_file()
    else {
        return;
    };
    write(window, with_extension(path));
}

fn write(window: &mut Window, path: std::path::PathBuf) {
    let project = captured(window);
    let digest = digest(&project.manifest);
    let written = save(&path, &project)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot write the project {}", path.display()));
    if window
        .machine
        .status
        .report(&format!("Saved {}", path.display()), written)
        .is_some()
    {
        window.doc.project.path = Some(path);
        window.doc.project.saved = Some(digest);
    }
}

/// The plate as a project, which is both what gets written and what gets hashed.
fn captured(window: &Window) -> Project {
    state::capture(state::Captured {
        scene: &window.doc.scene,
        slicing: &window.machine.slicing,
        supports: &window.tools.supports,
        hollow: &window.tools.hollow,
        drain: &window.tools.drain,
        cut: &window.tools.cut,
        array: &window.tools.array,
    })
}

/// The empty plate the window opens on counts as saved: closing it again asks nothing.
pub fn mark_saved(window: &mut Window) {
    window.doc.project.saved = Some(digest(&captured(window).manifest));
}

/// Whether the plate has moved since it was last written or read.
fn is_dirty(window: &Window) -> bool {
    window.doc.project.saved != Some(digest(&captured(window).manifest))
}

/// Holds the window open when there is work in it nobody has written down, and asks what
/// to do about it. Nothing is asked of a plate that matches its file.
pub fn guard_close(ui: &egui::Ui, window: &mut Window) {
    if window.doc.project.closing {
        return;
    }
    let asked = ui.ctx().input(|input| input.viewport().close_requested());
    if asked && !window.doc.project.asking {
        if !is_dirty(window) {
            return;
        }
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        window.doc.project.asking = true;
    }
    if !window.doc.project.asking {
        return;
    }

    let mut close = false;
    egui::Modal::new(egui::Id::new("unsaved")).show(ui.ctx(), |ui| {
        ui.label("This plate has changes that are not in a project file.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Save and close").clicked() {
                save_open(window);
                // A dialog the user backed out of leaves the plate unsaved, and the
                // window open with it.
                close = !is_dirty(window);
                window.doc.project.asking = false;
            }
            if ui.button("Close without saving").clicked() {
                close = true;
                window.doc.project.asking = false;
            }
            if ui.button("Keep working").clicked() {
                window.doc.project.asking = false;
                window.machine.updates.cancel_restart();
            }
        });
    });
    if close {
        window.doc.project.closing = true;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// A name typed without the extension is still a project file; a dialog that already
/// applied it is left alone.
fn with_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
    {
        path
    } else {
        path.with_extension(EXTENSION)
    }
}
