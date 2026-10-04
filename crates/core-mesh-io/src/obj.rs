use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::Path;

use core_geometry::{Mapping, Mesh, UvMap, Vec2, Vec3};

use crate::{Loaded, MeshIoError, MeshLoader, ModelFile, Texture};

/// Reads Wavefront OBJ. The format carries no units; values are taken as millimetres.
///
/// Every object and group in the file becomes part of one mesh: what is printed is the
/// whole file. The material file is read for one thing only, the `map_Kd` image the `vt`
/// coordinates address; see `docs/decisions/0115`.
pub struct ObjLoader;

impl MeshLoader for ObjLoader {
    fn extensions(&self) -> &'static [&'static str] {
        &["obj"]
    }

    fn read(&self, file: ModelFile<'_>) -> Result<Loaded, MeshIoError> {
        let (path, beside) = (file.path, file.beside);
        let mut reader = BufReader::new(file.source);

        let options = tobj::LoadOptions {
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
            ..tobj::LoadOptions::default()
        };
        let (models, materials) =
            tobj::load_obj_buf(&mut reader, &options, |mtl| load_mtl(path, mtl, beside)).map_err(
                |source| MeshIoError::Malformed {
                    path: path.to_owned(),
                    format: "OBJ",
                    reason: source.to_string(),
                },
            )?;

        let mesh = merge(&models);
        let (textures, of_material) = diffuse_maps(beside, materials.as_deref().unwrap_or(&[]));
        let uvs = uv_map(&models, mesh.faces.len(), &of_material);
        tracing::debug!(
            path = %path.display(),
            objects = models.len(),
            faces = mesh.faces.len(),
            textures = textures.len(),
            "loaded OBJ"
        );
        Ok(Loaded {
            mesh,
            uvs,
            textures,
        })
    }
}

/// Reads one `mtllib` beside the OBJ, ignoring one that is not there.
///
/// `tobj` splits the line on a space alone, so a file separating the keyword from the name
/// with a tab hands over an empty path; the model's own name with an `.mtl` extension is
/// where such a file sits anyway.
fn load_mtl(
    obj: &Path,
    mtl: &Path,
    beside: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> tobj::MTLLoadResult {
    let own = obj.with_extension("mtl");
    let found = [Some(mtl), own.file_name().map(Path::new)]
        .into_iter()
        .flatten()
        .filter(|name| !name.as_os_str().is_empty())
        .filter_map(Path::to_str)
        .find_map(beside);
    let Some(bytes) = found else {
        return Ok((Vec::new(), HashMap::new()));
    };
    tobj::load_mtl_buf(&mut Cursor::new(bytes))
}

/// Every `map_Kd` the materials name, read once each, and which of them each material
/// uses.
///
/// Two materials sharing an image share its entry, so a model whose parts repeat a texture
/// does not carry it twice.
fn diffuse_maps(
    beside: &dyn Fn(&str) -> Option<Vec<u8>>,
    materials: &[tobj::Material],
) -> (Vec<Texture>, Vec<Option<usize>>) {
    let mut textures: Vec<Texture> = Vec::new();
    let of_material = materials
        .iter()
        .map(|material| {
            let texture = diffuse_map(beside, material.diffuse_texture.as_deref()?)?;
            let at = textures
                .iter()
                .position(|already| already.name == texture.name)
                .unwrap_or_else(|| {
                    textures.push(texture);
                    textures.len() - 1
                });
            Some(at)
        })
        .collect();
    (textures, of_material)
}

/// One `map_Kd` read from beside the OBJ.
///
/// Exporters routinely write the absolute path the image had on the machine that made the
/// file, so a name that resolves to nothing is tried again as a bare file name beside the
/// model, which is where such an image actually travels.
fn diffuse_map(beside: &dyn Fn(&str) -> Option<Vec<u8>>, named: &str) -> Option<Texture> {
    for name in [named, file_name(named)] {
        if let Some(bytes) = beside(name) {
            return Some(Texture {
                name: name.to_owned(),
                bytes,
            });
        }
    }
    None
}

/// The last component of a path written in either slash, since a Windows path reaching a
/// Unix box is not split by `Path` at all.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The `vt` coordinates of every face, in the order `merge` put the faces in.
///
/// An object that names no `vt` leaves its own faces unmapped rather than unmapping the
/// whole file: a model built from parts routinely textures only some of them, and what is
/// not covered simply does not move. `tobj` answers a face with no `vt` with its
/// neighbour's, so an object is read as unmapped by its coordinates collapsing to one
/// point rather than by the indices; see ADR 0121. A `usemtl` splits the file into models,
/// so a model's own material is what says which image its faces are textured from.
fn uv_map(models: &[tobj::Model], faces: usize, of_material: &[Option<usize>]) -> Option<UvMap> {
    let mut mapped: Vec<Option<Mapping>> = Vec::with_capacity(faces);
    for model in models {
        let mesh = &model.mesh;
        let count = mesh.indices.len() / 3;
        let texture = mesh
            .material_id
            .and_then(|id| of_material.get(id).copied().flatten())
            .filter(|_| {
                mesh.texcoords.len() > 2 && mesh.texcoord_indices.len() == mesh.indices.len()
            });
        let Some(texture) = texture else {
            mapped.extend(std::iter::repeat_n(None, count));
            continue;
        };

        let pairs = mesh.texcoords.as_chunks::<2>().0;
        for face in mesh.texcoord_indices.as_chunks::<3>().0 {
            mapped.push(corners_of(face, pairs).map(|corners| Mapping { texture, corners }));
        }
    }
    (mapped.len() == faces && mapped.iter().any(Option::is_some)).then(|| UvMap::new(mapped))
}

/// The three coordinates one face's `vt` indices name, or `None` if any is out of range.
fn corners_of(face: &[u32; 3], pairs: &[[f32; 2]]) -> Option<[Vec2; 3]> {
    let mut at = [Vec2::ZERO; 3];
    for (slot, &index) in at.iter_mut().zip(face) {
        let pair = pairs.get(index as usize)?;
        *slot = Vec2::new(pair[0], pair[1]);
    }
    Some(at)
}

/// Concatenates every object's vertices and faces, shifting each object's indices.
fn merge(models: &[tobj::Model]) -> Mesh {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    for model in models {
        let base = vertices.len() as u32;
        vertices.extend(
            model
                .mesh
                .positions
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| Vec3::new(p[0], p[1], p[2])),
        );
        faces.extend(
            model
                .mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|f| [base + f[0], base + f[1], base + f[2]]),
        );
    }

    Mesh::new(vertices, faces)
}
