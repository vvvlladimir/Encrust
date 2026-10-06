//! Files the user hands the window, and how it asks for them.
//!
//! At the desk a file is a path and a dialog answers at once. In a browser nothing has a
//! path: a file is its bytes, and a dialog answers on a later frame, through [`picked`].

use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use core_format::ReadSeek;

/// A file given to the window by a dialog or a drop.
#[derive(Debug, Clone)]
pub enum Handed {
    /// On disk, with whatever it refers to beside it.
    Path(PathBuf),
    /// Its bytes and the name it was picked under, with the files picked together with it:
    /// a model's materials and textures.
    Bytes {
        name: PathBuf,
        bytes: Arc<[u8]>,
        beside: Arc<[(String, Arc<[u8]>)]>,
    },
}

impl Handed {
    /// The file on its own, with nothing picked beside it.
    pub fn bytes(name: impl Into<PathBuf>, bytes: Arc<[u8]>) -> Self {
        Self::Bytes {
            name: name.into(),
            bytes,
            beside: Arc::new([]),
        }
    }

    /// What the file is called: its path, or the name it was picked under.
    pub fn path(&self) -> &Path {
        match self {
            Self::Path(path) => path,
            Self::Bytes { name, .. } => name,
        }
    }

    /// The file, open to read from the start.
    pub fn reader(&self) -> io::Result<Box<dyn ReadSeek + Send>> {
        Ok(match self {
            Self::Path(path) => Box::new(io::BufReader::new(std::fs::File::open(path)?)),
            Self::Bytes { bytes, .. } => Box::new(Cursor::new(Arc::clone(bytes))),
        })
    }

    pub fn text(&self) -> io::Result<String> {
        let mut text = String::new();
        self.reader()?.read_to_string(&mut text)?;
        Ok(text)
    }

    /// A file picked with this one, by the name the model refers to it under. Only the
    /// last part of that name counts: a browser hands over files, not folders.
    pub fn beside(&self, reference: &str) -> Option<Vec<u8>> {
        let Self::Bytes { beside, .. } = self else {
            return None;
        };
        let wanted = Path::new(reference).file_name()?;
        beside
            .iter()
            .find(|(name, _)| Path::new(name).file_name() == Some(wanted))
            .map(|(_, bytes)| bytes.to_vec())
    }
}

/// What a dialog is asking for, which is also where its answer goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    Model,
    /// Either, picked from one dialog: what the user has on an empty plate is a file, not
    /// a decision about which kind of file it is.
    ModelOrProject,
    Project,
    SlicedFile,
    PrinterProfile,
    ResinProfile,
}

impl Wanted {
    /// The filters a dialog shows, each its name and the extensions under it. The first is
    /// the one the dialog opens on, so it is the widest.
    pub fn filters(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Self::Model => &[("Mesh", &MESHES)],
            Self::ModelOrProject => &[
                ("Model or project", &OPENABLE),
                ("Mesh", &MESHES),
                ("Encrust project", &PROJECTS),
            ],
            Self::Project => &[("Encrust project", &PROJECTS)],
            Self::SlicedFile => &[("Sliced file", &crate::sliced::EXTENSIONS)],
            Self::PrinterProfile => &[("Printer profile", &["toml"])],
            Self::ResinProfile => &[("Resin profile", &["toml"])],
        }
    }

    /// Every extension the dialog lets through, which is the widest of its filters.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        allow(
            dead_code,
            reason = "only a browser filters a pick by extension itself"
        )
    )]
    pub fn extensions(self) -> &'static [&'static str] {
        self.filters()
            .first()
            .map_or(&[][..], |(_, extensions)| *extensions)
    }

    /// Whether `path` is one of the files this dialog asked for.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        allow(
            dead_code,
            reason = "only a browser filters a pick by extension itself"
        )
    )]
    pub fn takes(self, path: &Path) -> bool {
        has_extension(path, self.extensions())
    }

    /// Whether a pick brings the files beside it: a mesh opens with its materials and its
    /// textures, and several meshes open at once.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        allow(dead_code, reason = "only a browser picks several files for one model")
    )]
    pub fn takes_several(self) -> bool {
        matches!(self, Self::Model | Self::ModelOrProject)
    }
}

/// The meshes a model is read from.
pub const MESHES: [&str; 3] = ["stl", "obj", "3mf"];

/// The project file, which is the one thing the window writes and reads back whole.
pub const PROJECTS: [&str; 1] = [core_engine::project::EXTENSION];

/// What the plate can be filled from in one dialog: a model or a project.
pub const OPENABLE: [&str; 4] = [
    MESHES[0],
    MESHES[1],
    MESHES[2],
    core_engine::project::EXTENSION,
];

/// Whether `path` ends in one of `extensions`, whatever case it was written in.
pub fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// What a mesh refers to and is read with: an `.obj`'s materials and its textures.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only a browser hands over bytes, and only later")
)]
pub const SIBLINGS: [&str; 6] = ["mtl", "png", "jpg", "jpeg", "bmp", "tga"];

/// Something a browser's dialog or download came back with.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only a browser hands over bytes, and only later")
)]
#[derive(Debug)]
pub enum Arrived {
    File(Wanted, Handed),
    /// Dropped on the window, to be opened by what it is.
    Dropped(Handed),
    Failed(String),
}

/// Files handed over at once, as the window opens them: each mesh with the files that
/// sit beside it in its own folder, and anything else on its own.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only a browser hands over bytes, and only later")
)]
pub fn together(files: Vec<(String, Arc<[u8]>)>) -> Vec<Handed> {
    let is = |name: &str, extensions: &[&str]| has_extension(Path::new(name), extensions);
    let (siblings, rest): (Vec<_>, Vec<_>) =
        files.into_iter().partition(|(name, _)| is(name, &SIBLINGS));
    rest.into_iter()
        .map(|(name, bytes)| Handed::Bytes {
            beside: if is(&name, &MESHES) {
                in_folder_of(&name, &siblings)
            } else {
                Arc::new([])
            },
            name: PathBuf::from(name),
            bytes,
        })
        .collect()
}

/// The siblings in the same folder as `mesh`, so two models dropped together each keep
/// their own `material.mtl`.
fn in_folder_of(mesh: &str, siblings: &[(String, Arc<[u8]>)]) -> Arc<[(String, Arc<[u8]>)]> {
    let folder = Path::new(mesh).parent();
    siblings
        .iter()
        .filter(|(name, _)| Path::new(name).parent() == folder)
        .cloned()
        .collect()
}

/// Asks for a file. At the desk the answer is returned; in a browser it is `None`, and the
/// file arrives through [`picked`] once the user has chosen it.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick(wanted: Wanted) -> Option<Handed> {
    let mut dialog = rfd::FileDialog::new();
    for (name, extensions) in wanted.filters() {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.pick_file().map(Handed::Path)
}

#[cfg(target_arch = "wasm32")]
pub fn pick(wanted: Wanted) -> Option<Handed> {
    crate::web::files::pick(wanted);
    None
}

/// What a browser's dialogs and downloads came back with since the last frame. Always
/// empty at the desk, where a dialog answers at once.
pub fn arrived() -> Vec<Arrived> {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::files::arrived()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_dialog_opens_both_a_model_and_a_project() {
        let wanted = Wanted::ModelOrProject;
        assert!(wanted.takes(Path::new("boat.STL")), "any case of a mesh");
        assert!(wanted.takes(Path::new("plate.encrust")));
        assert!(!wanted.takes(Path::new("plate.goo")));
        let names: Vec<&str> = wanted.filters().iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["Model or project", "Mesh", "Encrust project"]);
    }

    #[test]
    fn a_picked_file_reads_back_its_own_bytes() {
        let handed = Handed::bytes("cube.stl", Arc::from(&b"solid cube"[..]));
        assert_eq!(handed.path(), Path::new("cube.stl"));
        assert_eq!(handed.text().expect("bytes read"), "solid cube");
    }

    #[test]
    fn a_sibling_is_found_by_the_last_part_of_the_name_the_model_uses() {
        let handed = Handed::Bytes {
            name: PathBuf::from("boat.obj"),
            bytes: Arc::from(&b""[..]),
            beside: Arc::new([("hull.png".to_owned(), Arc::from(&b"png"[..]))]),
        };
        assert_eq!(handed.beside("textures/hull.png"), Some(b"png".to_vec()));
        assert_eq!(handed.beside("deck.png"), None);
    }

    #[test]
    fn files_handed_together_give_each_mesh_the_others_beside_it() {
        let bytes = || Arc::from(&b""[..]);
        let handed = together(vec![
            ("boat.obj".to_owned(), bytes()),
            ("boat.MTL".to_owned(), bytes()),
            ("hull.png".to_owned(), bytes()),
            ("plate.goo".to_owned(), bytes()),
        ]);

        let names: Vec<&Path> = handed.iter().map(Handed::path).collect();
        assert_eq!(names, [Path::new("boat.obj"), Path::new("plate.goo")]);
        assert!(
            handed[0].beside("boat.mtl").is_none(),
            "names match exactly"
        );
        assert!(handed[0].beside("boat.MTL").is_some());
        assert!(handed[0].beside("hull.png").is_some());
        assert!(
            handed[1].beside("hull.png").is_none(),
            "a sliced file has nothing beside it"
        );
    }

    #[test]
    fn models_handed_together_keep_their_own_siblings_of_one_name() {
        let handed = together(vec![
            ("boat/boat.obj".to_owned(), Arc::from(&b""[..])),
            ("boat/material.mtl".to_owned(), Arc::from(&b"boat"[..])),
            ("car/car.obj".to_owned(), Arc::from(&b""[..])),
            ("car/material.mtl".to_owned(), Arc::from(&b"car"[..])),
        ]);
        assert_eq!(handed[0].beside("material.mtl"), Some(b"boat".to_vec()));
        assert_eq!(handed[1].beside("material.mtl"), Some(b"car".to_vec()));
    }
}
