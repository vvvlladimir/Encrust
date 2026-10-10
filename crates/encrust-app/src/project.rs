//! The `.encrust` project file as the window handles it: the dialogs, and whether the
//! plate in hand matches the file it came from.
//!
//! The format itself — the manifest, the mesh blobs and their (de)serialisation — is
//! `core_engine::project`. What the window converts to and from it is `state`.

pub mod state;

use std::path::PathBuf;

use anyhow::Context as _;
use core_engine::project::{EXTENSION, Project, ProjectError, digest, load, read_from};

use crate::files::{self, Handed, Wanted};
use crate::panels::Window;
use crate::panels::frame_view;
use crate::scene::Scene;
use crate::status::Status;
use crate::undo::History;

/// The project file the window has open, so that Save knows where to write and the title
/// strip knows what to call the plate.
#[derive(Debug, Default)]
pub struct Opened {
    /// In a browser, where nothing has a path, the name it was last opened or saved under.
    pub path: Option<PathBuf>,
    /// The digest of the plate as it was last written or read, or `None` for a plate that
    /// has never been either. Unsaved work is this not matching the plate in hand.
    saved: Option<u64>,
    /// What the window is waiting to be told about work that is not saved, or `None`
    /// when it has asked nothing.
    asking: Option<Asking>,
    /// The answer was to save first, and the dialog that asks where runs on the next
    /// frame: a modal of our own is still on screen on this one.
    saving: Option<Asking>,
    /// The answer was to close anyway, so the next close request is not questioned again.
    closing: bool,
}

/// What the window is about to do to a plate nobody has written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asking {
    Close,
    NewProject,
}

impl Asking {
    /// What a button offers to do, and what the modal's line calls it.
    fn verb(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::NewProject => "start a new project",
        }
    }
}

impl Opened {
    /// What the top bar calls the plate: the file's own name, or nothing at all for a
    /// plate that has never been saved.
    pub fn name(&self) -> Option<&str> {
        self.path.as_ref()?.file_stem()?.to_str()
    }
}

/// Starts a project from nothing, asking first about work that is not written down.
pub fn new_project(window: &mut Window) {
    if is_dirty(window) {
        window.doc.project.asking = Some(Asking::NewProject);
        return;
    }
    clear(window);
}

/// Takes everything off the plate and forgets which file it came from. The chosen
/// profiles and the tools' own numbers stay: they are how this user works, not what is
/// on the plate.
pub fn clear(window: &mut Window) {
    window.doc.scene = Scene::default();
    window.doc.history = History::default();
    window.doc.project.path = None;
    window.doc.project.saved = Some(digest(&captured(window).manifest));
    window.machine.status = Status::Info("New project".to_owned());
}

/// Asks for a project and opens it.
pub fn open_dialog(window: &mut Window) {
    if let Some(file) = files::pick(Wanted::Project) {
        open(window, &file);
    }
}

/// Opens a project over whatever the window was holding.
pub fn open(window: &mut Window, file: &Handed) {
    let path = file.path().to_path_buf();
    let read = match file {
        Handed::Path(path) => load(path),
        Handed::Bytes { bytes, .. } => read_from(std::io::Cursor::new(&bytes[..])),
    };
    let read = read
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot open the project {}", path.display()));
    let Some(project) = window
        .machine
        .status
        .report(&format!("Opened {}", path.display()), read)
    else {
        return;
    };

    state::apply(
        project,
        state::CapturedMut {
            scene: &mut window.doc.scene,
            slicing: &mut window.machine.slicing,
            tools: window.tools,
        },
    );
    window.machine.recent.note(&path, crate::updates::now_s());
    window.doc.project.path = Some(path);
    window.doc.project.saved = Some(digest(&captured(window).manifest));
    window.doc.history = History::default();
    frame_view(
        &window.doc.scene,
        &window.doc.plate,
        &mut window.view.camera,
    );
}

/// Writes over the file the plate came from, or asks for a name when it came from none.
/// Answers whether the plate is on disk afterwards.
pub fn save_open(window: &mut Window) -> bool {
    match window.doc.project.path.clone() {
        Some(path) => write(window, path),
        None => save_dialog(window),
    }
}

/// Asks for a name and writes the plate under it. Answers whether it was written: a
/// dialog the user backed out of writes nothing.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_dialog(window: &mut Window) -> bool {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Encrust project", &[EXTENSION])
        .set_file_name(format!("plate.{EXTENSION}"))
        .save_file()
    else {
        return false;
    };
    write(window, with_extension(path))
}

/// A browser asks where to put a download itself, so the project goes out under the name
/// it came in with.
#[cfg(target_arch = "wasm32")]
pub fn save_dialog(window: &mut Window) -> bool {
    let name = window
        .doc
        .project
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("plate.{EXTENSION}")));
    write(window, name)
}

fn write(window: &mut Window, path: PathBuf) -> bool {
    let project = captured(window);
    let digest = digest(&project.manifest);
    let written = store(&path, &project)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot write the project {}", path.display()));
    let saved = window
        .machine
        .status
        .report(&format!("Saved {}", path.display()), written)
        .is_some();
    if saved {
        window.machine.recent.note(&path, crate::updates::now_s());
        window.doc.project.path = Some(path);
        window.doc.project.saved = Some(digest);
    }
    saved
}

#[cfg(not(target_arch = "wasm32"))]
fn store(path: &std::path::Path, project: &Project) -> Result<(), ProjectError> {
    core_engine::project::save(path, project)
}

/// Written whole in memory and handed to the user as a download.
#[cfg(target_arch = "wasm32")]
fn store(path: &std::path::Path, project: &Project) -> Result<(), ProjectError> {
    let mut sink = std::io::Cursor::new(Vec::new());
    core_engine::project::write_to(&mut sink, project)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    crate::web::files::download(&name, sink.get_ref())
        .map_err(|error| ProjectError::Io(crate::web::opfs::failed(error)))
}

/// The plate as a project, which is both what gets written and what gets hashed.
fn captured(window: &Window) -> Project {
    state::capture(state::Captured {
        scene: &window.doc.scene,
        slicing: &window.machine.slicing,
        tools: window.tools,
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
/// to do about it — on the way out, and on the way to a new project. Nothing is asked of
/// a plate that matches its file.
pub fn guard_close(ui: &egui::Ui, window: &mut Window) {
    if window.doc.project.closing {
        return;
    }
    // The question holds the window open for as long as it stands: every close asked for
    // under it is cancelled again, not only the first.
    let waiting = window.doc.project.asking.is_some() || window.doc.project.saving.is_some();
    if ui.ctx().input(|input| input.viewport().close_requested()) {
        if !waiting && !is_dirty(window) {
            return;
        }
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        window.doc.project.asking.get_or_insert(Asking::Close);
    }
    // A dialog the user backed out of writes nothing, and the plate stays as it is with it.
    if let Some(asking) = window.doc.project.saving.take() {
        if save_open(window) {
            go_on(ui.ctx(), window, asking);
        }
        return;
    }
    let Some(asking) = window.doc.project.asking else {
        return;
    };

    let mut answered = None;
    egui::Modal::new(egui::Id::new("unsaved")).show(ui.ctx(), |ui| {
        ui.label("This plate has changes that are not in a project file.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let verb = asking.verb();
            if ui.button(format!("Save and {verb}")).clicked() {
                // Where to write is asked on the next frame, once this modal is gone:
                // a file dialog opened from under it never comes back.
                window.doc.project.saving = Some(asking);
                window.doc.project.asking = None;
                ui.ctx().request_repaint();
            }
            if ui
                .button(format!("{} without saving", first_capital(verb)))
                .clicked()
            {
                answered = Some(asking);
                window.doc.project.asking = None;
            }
            if ui.button("Keep working").clicked() {
                window.doc.project.asking = None;
                window.machine.updates.cancel_restart();
            }
        });
    });
    if let Some(asking) = answered {
        go_on(ui.ctx(), window, asking);
    }
}

/// Does what the plate was held back from: closes the window, or empties it.
fn go_on(ctx: &egui::Context, window: &mut Window, asking: Asking) {
    match asking {
        Asking::Close => {
            window.doc.project.closing = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        Asking::NewProject => clear(window),
    }
}

/// A label that starts a button, from a verb that starts a sentence.
fn first_capital(verb: &str) -> String {
    let mut letters = verb.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

/// A name typed without the extension is still a project file; a dialog that already
/// applied it is left alone.
#[cfg(not(target_arch = "wasm32"))]
fn with_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
    {
        path
    } else {
        path.with_extension(EXTENSION)
    }
}
