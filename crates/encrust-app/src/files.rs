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
    Project,
    SlicedFile,
    PrinterProfile,
    ResinProfile,
}

impl Wanted {
    /// The filter a dialog shows: its name and the extensions under it.
    pub fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Model => ("Mesh", &MESHES),
            Self::Project => ("Encrust project", &[core_engine::project::EXTENSION]),
            Self::SlicedFile => ("Sliced file", &crate::sliced::EXTENSIONS),
            Self::PrinterProfile => ("Printer profile", &["toml"]),
            Self::ResinProfile => ("Resin profile", &["toml"]),
        }
    }
}

/// The meshes a model is read from.
pub const MESHES: [&str; 3] = ["stl", "obj", "3mf"];

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
/// sit beside one, and anything else on its own.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only a browser hands over bytes, and only later")
)]
pub fn together(files: Vec<(String, Arc<[u8]>)>) -> Vec<Handed> {
    let is = |name: &str, extensions: &[&str]| {
        Path::new(name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extensions
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(extension))
            })
    };
    let beside: Arc<[(String, Arc<[u8]>)]> = files
        .iter()
        .filter(|(name, _)| is(name, &SIBLINGS))
        .cloned()
        .collect();
    files
        .into_iter()
        .filter(|(name, _)| !is(name, &SIBLINGS))
        .map(|(name, bytes)| Handed::Bytes {
            beside: if is(&name, &MESHES) {
                Arc::clone(&beside)
            } else {
                Arc::new([])
            },
            name: PathBuf::from(name),
            bytes,
        })
        .collect()
}

/// Asks for a file. At the desk the answer is returned; in a browser it is `None`, and the
/// file arrives through [`picked`] once the user has chosen it.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick(wanted: Wanted) -> Option<Handed> {
    let (name, extensions) = wanted.filter();
    rfd::FileDialog::new()
        .add_filter(name, extensions)
        .pick_file()
        .map(Handed::Path)
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
}
