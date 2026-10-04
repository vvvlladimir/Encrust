use std::io::BufReader;

use core_geometry::{Mesh, Vec3};

use crate::{Loaded, MeshIoError, MeshLoader, ModelFile};

/// Reads binary and ASCII STL. The format carries no units; values are taken as millimetres.
///
/// Face normals stored in the file are ignored: exporters write them inconsistently, so
/// winding order is the only trustworthy source of orientation.
pub struct StlLoader;

impl MeshLoader for StlLoader {
    fn extensions(&self) -> &'static [&'static str] {
        &["stl"]
    }

    fn read(&self, file: ModelFile<'_>) -> Result<Loaded, MeshIoError> {
        let path = file.path;
        let mut reader = BufReader::new(file.source);

        let triangles =
            stl_io::create_stl_reader(&mut reader).map_err(|source| MeshIoError::Malformed {
                path: path.to_owned(),
                format: "STL",
                reason: source.to_string(),
            })?;

        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for triangle in triangles {
            let triangle = triangle.map_err(|source| MeshIoError::Malformed {
                path: path.to_owned(),
                format: "STL",
                reason: source.to_string(),
            })?;

            let base = vertices.len() as u32;
            for vertex in &triangle.vertices {
                vertices.push(Vec3::new(vertex[0], vertex[1], vertex[2]));
            }
            faces.push([base, base + 1, base + 2]);
        }

        let mesh = Mesh::new(vertices, faces);
        tracing::debug!(
            path = %path.display(),
            faces = mesh.faces.len(),
            "loaded STL"
        );
        Ok(Loaded::plain(mesh))
    }
}
