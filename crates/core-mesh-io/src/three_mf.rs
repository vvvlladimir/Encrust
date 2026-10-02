use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use core_geometry::{Mapping, Mat4, Mesh, UvMap, Vec2, Vec3};
use threemf2::model::domain::component::Component;
use threemf2::model::domain::mesh::{Mesh as ThreeMfMesh, Triangle};
use threemf2::model::domain::model::{Model, Unit};
use threemf2::model::domain::object::{Object, ObjectKind};
use threemf2::model::domain::transform::Transform;
use threemf2::model::domain::types::{PathResource, ResourceId};
use threemf2::package::ThreemfPackage;

use crate::{Loaded, MeshIoError, MeshLoader, Texture};

/// How deep a chain of components may nest before the file is called malformed. The
/// specification forbids a cycle; nothing in a file stops one.
const MAX_DEPTH: usize = 64;

/// Most bytes one part of the container may say it expands to.
///
/// A 3MF is a zip, and every reader of one — ours and the one under it — sizes a buffer
/// from the uncompressed length an entry states about itself. A model of a hundred
/// million triangles is well under this; see `docs/design/hostile-files.md`.
const MAX_PART_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Reads 3MF: the zip container's `3D/3dmodel.model`, its build items, and the objects
/// they reference through any depth of components.
///
/// The whole build becomes one mesh, placed where the file says and scaled from the
/// model's `unit` into millimetres. The materials extension's `texture2dgroup` becomes the
/// UVs beside it and the image part it names becomes the texture; see
/// `docs/decisions/0115-uvs-are-a-sidecar-beside-the-mesh.md`.
pub struct ThreeMfLoader;

impl MeshLoader for ThreeMfLoader {
    fn extensions(&self) -> &'static [&'static str] {
        &["3mf"]
    }

    fn load(&self, path: &Path) -> Result<Loaded, MeshIoError> {
        let file = File::open(path).map_err(|source| MeshIoError::Io {
            path: path.to_owned(),
            source,
        })?;
        refuse_oversized_parts(path, &file)?;

        let package = ThreemfPackage::from_reader_with_memory_optimized_deserializer(
            BufReader::new(file),
            true,
        )
        .map_err(|source| malformed(path, source.to_string()))?;

        let root = Mat4::from_scale(Vec3::splat(unit_scale(package.root.unit.as_ref())));
        let build = Build {
            package: &package,
            path,
        };

        let mut collected = Collected::default();
        for item in &package.root.build.item {
            let model = build.model(item.path.as_ref())?;
            let place = root * placement(item.transform.as_ref());
            build.append(model, item.objectid, place, 0, &mut collected)?;
        }

        let (uvs, textures) = collected.mapping(&package);
        tracing::debug!(
            path = %path.display(),
            items = package.root.build.item.len(),
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

/// Refuses an archive that claims a part larger than `MAX_PART_BYTES`.
///
/// Every claim is read before any part is, because the reader under us reserves a buffer
/// from it: a kilobyte of zip stating a four-gigabyte part is a bomb, not a model. The
/// entries are opened raw, so this costs a seek each and decompresses nothing.
fn refuse_oversized_parts(path: &Path, file: &File) -> Result<(), MeshIoError> {
    let file = file.try_clone().map_err(|source| MeshIoError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|source| malformed(path, source.to_string()))?;

    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|source| malformed(path, source.to_string()))?;
        if entry.size() > MAX_PART_BYTES {
            return Err(malformed(
                path,
                format!("the part {} claims {} bytes", entry.name(), entry.size()),
            ));
        }
    }
    Ok(())
}

/// One build walked into a mesh, with the texture coordinates found beside it.
#[derive(Default)]
struct Collected {
    mesh: Mesh,
    /// Per face of `mesh`, what it is textured from, if it named a texture group.
    faces: Vec<Option<Mapping>>,
    /// The image parts the groups named, in the order `faces` indexes them.
    parts: Vec<PathResource>,
}

impl Collected {
    /// Where a face textured from the part at `at` points, adding the part if this is the
    /// first face to name it.
    fn texture(&mut self, at: &PathResource) -> usize {
        if let Some(already) = self.parts.iter().position(|seen| seen == at) {
            return already;
        }
        self.parts.push(at.clone());
        self.parts.len() - 1
    }

    /// The map and the images it addresses, or no map when no face carries one. A face
    /// naming no texture group is left unmapped rather than unmapping the file.
    fn mapping(&self, package: &ThreemfPackage) -> (Option<UvMap>, Vec<Texture>) {
        if !self.faces.iter().any(Option::is_some) {
            return (None, Vec::new());
        }
        // A group whose image is not in the package leaves its faces unmapped: an index
        // into a list of images has to name one.
        let read: Vec<Option<Texture>> = self.parts.iter().map(|at| part(package, at)).collect();
        let mut kept = Vec::new();
        let at: Vec<Option<usize>> = read
            .into_iter()
            .map(|texture| {
                kept.push(texture?);
                Some(kept.len() - 1)
            })
            .collect();

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
        if faces.iter().any(Option::is_some) {
            (Some(UvMap::new(faces)), kept)
        } else {
            (None, Vec::new())
        }
    }
}

/// The image part at `at`, if the package carries its bytes.
fn part(package: &ThreemfPackage, at: &PathResource) -> Option<Texture> {
    let bytes = package.unknown_parts.get(at)?.clone();
    Some(Texture {
        name: at.as_str().to_owned(),
        bytes,
    })
}

/// One package being walked, and the path it came from so an error can name it.
struct Build<'a> {
    package: &'a ThreemfPackage,
    path: &'a Path,
}

impl Build<'_> {
    /// The root model, or the sub-model a build item or component points at.
    fn model(&self, at: Option<&PathResource>) -> Result<&Model, MeshIoError> {
        let Some(at) = at else {
            return Ok(&self.package.root);
        };
        self.package
            .sub_models
            .get(at)
            .ok_or_else(|| malformed(self.path, format!("no model part at {}", at.as_str())))
    }

    /// Adds one object's triangles to `out`, following components to the meshes below.
    fn append(
        &self,
        model: &Model,
        id: ResourceId,
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
            .resources
            .object
            .iter()
            .find(|object| object.id == id)
            .ok_or_else(|| malformed(self.path, format!("no object with id {id}")))?;

        match object.kind.as_ref() {
            Some(ObjectKind::Mesh(mesh)) => self.push(model, object, mesh, place, out)?,
            Some(ObjectKind::Components(components)) => {
                for component in &components.component {
                    self.follow(component, place, depth, out)?;
                }
            }
            Some(ObjectKind::BooleanShape(_)) => {
                return Err(MeshIoError::Unimplemented("3MF boolean shapes"));
            }
            Some(ObjectKind::DisplacementMesh(_)) => {
                return Err(MeshIoError::Unimplemented("3MF displacement meshes"));
            }
            // `ObjectKind` is `#[non_exhaustive]`, so a kind added upstream must not vanish.
            Some(_) => return Err(MeshIoError::Unimplemented("this 3MF object kind")),
            None => {}
        }
        Ok(())
    }

    fn follow(
        &self,
        component: &Component,
        place: Mat4,
        depth: usize,
        out: &mut Collected,
    ) -> Result<(), MeshIoError> {
        let model = self.model(component.path.as_ref())?;
        let place = place * placement(component.transform.as_ref());
        self.append(model, component.objectid, place, depth + 1, out)
    }

    fn push(
        &self,
        model: &Model,
        object: &Object,
        mesh: &ThreeMfMesh,
        place: Mat4,
        out: &mut Collected,
    ) -> Result<(), MeshIoError> {
        let base = out.mesh.vertices.len() as u32;
        let count = mesh.vertices.vertex.len() as u32;
        // A mirroring placement turns every face inside out, so the winding is put back.
        let mirrored = place.determinant() < 0.0;

        out.mesh
            .vertices
            .extend(mesh.vertices.vertex.iter().map(|vertex| {
                let point = Vec3::new(
                    vertex.x.value() as f32,
                    vertex.y.value() as f32,
                    vertex.z.value() as f32,
                );
                place.transform_point3(point)
            }));

        for triangle in &mesh.triangles.triangle {
            let [i, j, k] = [triangle.v1, triangle.v2, triangle.v3];
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

/// What one triangle is textured from, through its own `pid` or its object's.
///
/// The group's image is recorded on the way past, so a build whose parts name several
/// images comes out with one entry each.
fn mapped(
    model: &Model,
    object: &Object,
    triangle: &Triangle,
    out: &mut Collected,
) -> Option<Mapping> {
    let pid = triangle.pid.get().or_else(|| object.pid.get())?;
    let group = model
        .resources
        .texture2dgroup
        .iter()
        .find(|group| group.id == pid)?;
    let at = model
        .resources
        .texture2d
        .iter()
        .find(|texture| texture.id == group.texid)
        .map(|texture| texture.path.clone())?;

    let texture = out.texture(&at);

    let corner = |index: threemf2::model::domain::types::OptionalResourceIndex| {
        let coordinate = group.tex2coord.get(index.get()? as usize)?;
        Some(Vec2::new(
            coordinate.u.value() as f32,
            coordinate.v.value() as f32,
        ))
    };
    Some(Mapping {
        texture,
        corners: [
            corner(triangle.p1)?,
            corner(triangle.p2)?,
            corner(triangle.p3)?,
        ],
    })
}

/// 3MF multiplies a row vector by a 4x3 matrix, so its rows are glam's columns.
fn placement(transform: Option<&Transform>) -> Mat4 {
    let Some(transform) = transform else {
        return Mat4::IDENTITY;
    };
    let m = transform.0.map(|value| value as f32);
    Mat4::from_cols_array(&[
        m[0], m[1], m[2], 0.0, //
        m[3], m[4], m[5], 0.0, //
        m[6], m[7], m[8], 0.0, //
        m[9], m[10], m[11], 1.0,
    ])
}

/// Millimetres per unit of the model's own `unit` attribute.
fn unit_scale(unit: Option<&Unit>) -> f32 {
    match unit.unwrap_or(&Unit::Millimeter) {
        Unit::Micron => 0.001,
        Unit::Centimeter => 10.0,
        Unit::Inch => 25.4,
        Unit::Foot => 304.8,
        Unit::Meter => 1000.0,
        Unit::Millimeter => 1.0,
    }
}

fn malformed(path: &Path, reason: String) -> MeshIoError {
    MeshIoError::Malformed {
        path: path.to_owned(),
        format: "3MF",
        reason,
    }
}
