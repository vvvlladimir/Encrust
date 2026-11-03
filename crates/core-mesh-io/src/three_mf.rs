mod model;
mod package;
mod parse;

use std::io::SeekFrom;
use std::path::Path;

use core_geometry::{FastMap, Mapping, Mat4, Mesh, UvMap, Vec3};

use self::model::{MeshPart, Model, Object, Placed, Shape, Triangle};
use self::package::Package;
use crate::{Loaded, MeshIoError, MeshLoader, ModelFile, Texture};

/// How deep a chain of components may nest before the file is called malformed. The
/// specification forbids a cycle; nothing in a file stops one.
const MAX_DEPTH: usize = 64;

/// Reads a 3MF package's build into one mesh in millimetres; texture groups become UVs and
/// textures (ADR 0115, 0395).
pub struct ThreeMfLoader;

impl MeshLoader for ThreeMfLoader {
    fn extensions(&self) -> &'static [&'static str] {
        &["3mf"]
    }

    fn read(&self, file: ModelFile<'_>) -> Result<Loaded, MeshIoError> {
        let path = file.path;
        let file_bytes = file
            .source
            .seek(SeekFrom::End(0))
            .map_err(|source| MeshIoError::Io {
                path: path.to_owned(),
                source,
            })?;
        let mut package =
            Package::open(file.source, file_bytes).map_err(|reason| malformed(path, reason))?;
        let parts = Parts::read(&mut package).map_err(|reason| malformed(path, reason))?;

        let root = Mat4::from_scale(Vec3::splat(parts.root.mm_per_unit));
        let build = Build {
            parts: &parts,
            path,
        };
        let mut collected = Collected::default();
        for item in &parts.root.build {
            let model = build.model(item.path.as_deref())?;
            build.append(
                model,
                item.objectid,
                root * item.transform,
                0,
                &mut collected,
            )?;
        }

        let (uvs, textures) = collected
            .mapping(&mut package)
            .map_err(|reason| malformed(path, reason))?;
        tracing::debug!(
            path = %path.display(),
            items = parts.root.build.len(),
            faces = collected.mesh.faces.len(),
            mapped = uvs.is_some(),
            textures = textures.len(),
            "loaded 3MF"
        );
        Ok(Loaded {
            mesh: collected.mesh,
            uvs,
            textures,
        })
    }
}

/// The root model part and every part its build and components name. Only the root may
/// name another part, so one level is all there is.
struct Parts {
    root_name: String,
    root: Model,
    others: FastMap<String, Model>,
}

impl Parts {
    fn read(package: &mut Package<'_>) -> Result<Self, String> {
        let written = package.root_name()?;
        let root = package.model(&written, true)?;
        let root_name = package::part_name(&written);
        let components = root
            .objects
            .values()
            .flat_map(|object| match &object.shape {
                Shape::Components(components) => components.as_slice(),
                _ => &[],
            });
        let mut others = FastMap::default();
        for named in root.build.iter().chain(components) {
            let Some(written) = named.path.as_deref() else {
                continue;
            };
            let at = package::part_name(written);
            if at != root_name && !others.contains_key(&at) {
                let model = package.model(written, false)?;
                others.insert(at, model);
            }
        }
        Ok(Self {
            root_name,
            root,
            others,
        })
    }
}

/// One build walked into a mesh, with the texture coordinates found beside it.
#[derive(Default)]
struct Collected {
    mesh: Mesh,
    /// Per face of `mesh`, what it is textured from, if it named a texture group.
    faces: Vec<Option<Mapping>>,
    /// The image parts the groups named, in the order `faces` indexes them.
    parts: Vec<String>,
}

impl Collected {
    /// Where a face textured from the part at `at` points, adding the part if this is the
    /// first face to name it.
    fn texture(&mut self, at: &str) -> usize {
        if let Some(already) = self.parts.iter().position(|seen| seen == at) {
            return already;
        }
        self.parts.push(at.to_owned());
        self.parts.len() - 1
    }

    /// The map and the images it addresses, or no map when no face carries one. A face
    /// naming no texture group is left unmapped rather than unmapping the file.
    fn mapping(&self, package: &mut Package<'_>) -> Result<(Option<UvMap>, Vec<Texture>), String> {
        if !self.faces.iter().any(Option::is_some) {
            return Ok((None, Vec::new()));
        }
        // A group whose image is not in the package leaves its faces unmapped: an index
        // into a list of images has to name one.
        let mut kept = Vec::new();
        let mut at = Vec::with_capacity(self.parts.len());
        for name in &self.parts {
            at.push(package.bytes(name)?.map(|bytes| {
                kept.push(Texture {
                    name: name.clone(),
                    bytes,
                });
                kept.len() - 1
            }));
        }

        let faces = self
            .faces
            .iter()
            .map(|face| {
                let mapping = (*face)?;
                Some(Mapping {
                    texture: (*at.get(mapping.texture)?)?,
                    ..mapping
                })
            })
            .collect::<Vec<_>>();
        Ok(if faces.iter().any(Option::is_some) {
            (Some(UvMap::new(faces)), kept)
        } else {
            (None, Vec::new())
        })
    }
}

/// The parts being walked, and the path they came from so an error can name it.
struct Build<'a> {
    parts: &'a Parts,
    path: &'a Path,
}

impl Build<'_> {
    /// The root model, or the model part a build item or component names.
    fn model(&self, at: Option<&str>) -> Result<&Model, MeshIoError> {
        let Some(at) = at.map(package::part_name) else {
            return Ok(&self.parts.root);
        };
        if at == self.parts.root_name {
            return Ok(&self.parts.root);
        }
        self.parts
            .others
            .get(&at)
            .ok_or_else(|| malformed(self.path, format!("no model part at {at}")))
    }

    /// Adds one object's triangles to `out`, following components to the meshes below.
    fn append(
        &self,
        model: &Model,
        id: u32,
        place: Mat4,
        depth: usize,
        out: &mut Collected,
    ) -> Result<(), MeshIoError> {
        if depth > MAX_DEPTH {
            return Err(malformed(
                self.path,
                format!("components nest more than {MAX_DEPTH} deep"),
            ));
        }
        let object = model
            .objects
            .get(&id)
            .ok_or_else(|| malformed(self.path, format!("no object with id {id}")))?;

        match &object.shape {
            Shape::Mesh(mesh) => self.push(model, object, mesh, place, out),
            Shape::Components(components) => components
                .iter()
                .try_for_each(|component| self.follow(model, component, place, depth, out)),
            Shape::Unsupported(kind) => Err(MeshIoError::Unimplemented(kind)),
            Shape::Empty => Ok(()),
        }
    }

    /// A component without a path places an object of the model it sits in.
    fn follow(
        &self,
        model: &Model,
        component: &Placed,
        place: Mat4,
        depth: usize,
        out: &mut Collected,
    ) -> Result<(), MeshIoError> {
        let model = match component.path.as_deref() {
            Some(at) => self.model(Some(at))?,
            None => model,
        };
        let place = place * component.transform;
        self.append(model, component.objectid, place, depth + 1, out)
    }

    fn push(
        &self,
        model: &Model,
        object: &Object,
        mesh: &MeshPart,
        place: Mat4,
        out: &mut Collected,
    ) -> Result<(), MeshIoError> {
        let base = out.mesh.vertices.len() as u32;
        let count = mesh.vertices.len() as u32;
        // A mirroring placement turns every face inside out, so the winding is put back.
        let mirrored = place.determinant() < 0.0;

        out.mesh.vertices.extend(
            mesh.vertices
                .iter()
                .map(|vertex| place.transform_point3(*vertex)),
        );

        for triangle in &mesh.triangles {
            let [i, j, k] = triangle.vertices;
            if i >= count || j >= count || k >= count {
                return Err(malformed(
                    self.path,
                    format!("a triangle names vertex {} of {count}", i.max(j).max(k)),
                ));
            }
            let face = if mirrored { [i, k, j] } else { [i, j, k] };
            out.mesh.faces.push(face.map(|index| base + index));

            let mapping = mapped(model, object, triangle, out).map(|mut mapping| {
                if mirrored {
                    mapping.corners.swap(1, 2);
                }
                mapping
            });
            out.faces.push(mapping);
        }
        Ok(())
    }
}

/// What one triangle is textured from, through its own `pid` or its object's; the image is
/// recorded on the way past, so each one a build names comes out once.
fn mapped(
    model: &Model,
    object: &Object,
    triangle: &Triangle,
    out: &mut Collected,
) -> Option<Mapping> {
    let group = model.groups.get(&triangle.pid.or(object.pid)?)?;
    let at = model.textures.get(&group.texture)?;
    let corners =
        corners(object, triangle)?.map(|index| group.coordinates.get(index as usize).copied());
    let corners = [corners[0]?, corners[1]?, corners[2]?];
    Some(Mapping {
        texture: out.texture(at),
        corners,
    })
}

/// The group indices of a triangle's corners. Without `p2` or `p3`, `p1` holds for the whole
/// triangle, and without `p1` the object's `pindex` does.
fn corners(object: &Object, triangle: &Triangle) -> Option<[u32; 3]> {
    match triangle.properties {
        [Some(p1), Some(p2), Some(p3)] => Some([p1, p2, p3]),
        [p1, _, _] => Some([p1.or(object.pindex)?; 3]),
    }
}

fn malformed(path: &Path, reason: String) -> MeshIoError {
    MeshIoError::Malformed {
        path: path.to_owned(),
        format: "3MF",
        reason,
    }
}
