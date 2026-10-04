use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::{Component, Path};

use crate::{Loaded, MeshIoError, ObjLoader, StlLoader, ThreeMfLoader};

/// A byte source a loader can read and rewind.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

/// A model file in hand: its name, its bytes, and a way to the files that sit beside it.
///
/// Nothing here touches a disk, so a browser hands over what the user picked the same way
/// a desktop hands over a path.
pub struct ModelFile<'a> {
    /// What the file is called, which is what errors name and what a sibling is found
    /// beside.
    pub path: &'a Path,
    pub source: &'a mut dyn ReadSeek,
    /// The bytes of a file next to this one by its name as the model refers to it — an
    /// `.obj`'s material library or its texture — or `None` when there is no such file.
    pub beside: &'a dyn Fn(&str) -> Option<Vec<u8>>,
}

/// A mesh importer for one file format.
///
/// Loaders return the file's triangles as they are stored, with no vertices shared
/// between faces unless the format itself shares them. Call [`core_geometry::weld`]
/// before asking anything about the topology.
pub trait MeshLoader {
    /// Lower-case extensions this loader accepts, without the dot.
    fn extensions(&self) -> &'static [&'static str];

    fn read(&self, file: ModelFile<'_>) -> Result<Loaded, MeshIoError>;

    /// Reads the file at `path`, finding its siblings in the same directory.
    fn load(&self, path: &Path) -> Result<Loaded, MeshIoError> {
        let file = File::open(path).map_err(|source| MeshIoError::Io {
            path: path.to_owned(),
            source,
        })?;
        let directory = path.parent().unwrap_or(Path::new("."));
        let beside = |name: &str| read_beside(directory, name);
        self.read(ModelFile {
            path,
            source: &mut BufReader::new(file),
            beside: &beside,
        })
    }
}

/// A file the model names, read only when it lies within `directory`.
///
/// The name comes from the model's own bytes, so an absolute path, a drive or share prefix
/// or a `..` could otherwise read any file the user can.
fn read_beside(directory: &Path, name: &str) -> Option<Vec<u8>> {
    let name = Path::new(name);
    let within = name
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    within.then(|| std::fs::read(directory.join(name)).ok())?
}

/// Picks a loader by file extension, case-insensitively.
pub fn loader_for_extension(extension: &str) -> Result<Box<dyn MeshLoader>, MeshIoError> {
    let extension = extension.to_ascii_lowercase();
    let loaders: [Box<dyn MeshLoader>; 3] = [
        Box::new(StlLoader),
        Box::new(ObjLoader),
        Box::new(ThreeMfLoader),
    ];

    loaders
        .into_iter()
        .find(|loader| loader.extensions().contains(&extension.as_str()))
        .ok_or(MeshIoError::UnsupportedExtension(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
    }

    #[test]
    fn a_sibling_in_the_directory_is_read() {
        assert!(read_beside(fixtures(), "textured_quad.obj").is_some());
    }

    #[test]
    fn an_absolute_name_is_not_read() {
        let outside = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        assert!(read_beside(fixtures(), outside).is_none());
    }

    #[test]
    fn a_name_climbing_out_of_the_directory_is_not_read() {
        assert!(read_beside(fixtures(), "../../Cargo.toml").is_none());
    }

    #[test]
    fn extension_lookup_ignores_case() {
        let loader = loader_for_extension("STL").expect("stl is supported");
        assert_eq!(loader.extensions(), &["stl"]);
    }

    #[test]
    fn unknown_extension_is_rejected() {
        let Err(err) = loader_for_extension("gcode") else {
            panic!("gcode is not a mesh format");
        };
        assert!(matches!(err, MeshIoError::UnsupportedExtension(ext) if ext == "gcode"));
    }
}
