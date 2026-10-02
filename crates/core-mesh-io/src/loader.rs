use std::path::Path;

use crate::{Loaded, MeshIoError, ObjLoader, StlLoader, ThreeMfLoader};

/// A mesh importer for one file format.
///
/// Loaders return the file's triangles as they are stored, with no vertices shared
/// between faces unless the format itself shares them. Call [`core_geometry::weld`]
/// before asking anything about the topology.
pub trait MeshLoader {
    /// Lower-case extensions this loader accepts, without the dot.
    fn extensions(&self) -> &'static [&'static str];

    fn load(&self, path: &Path) -> Result<Loaded, MeshIoError>;
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
