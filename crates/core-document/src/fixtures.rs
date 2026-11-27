//! The plates the tests edit.

use std::path::Path;
use std::sync::Arc;

use core_edit::ObjectId;
use core_engine::project::{Project, load};
use core_geometry::{Mesh, Transform, Vec3, cuboid};

/// A project of version 4, before ids, holding one model named "cube".
const CUBE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cube.encrust");

/// The side of [`cube`]'s cube, millimetres.
pub const SIDE_MM: f32 = 10.0;

/// A plate of one solid cube named "cube", [`SIDE_MM`] on a side, standing on the plate at
/// the origin: the project file's manifest round a body of known volume.
pub fn cube() -> Project {
    cube_of(SIDE_MM)
}

/// The same plate round a cube `side_mm` on a side.
pub fn cube_of(side_mm: f32) -> Project {
    let mut project = load(Path::new(CUBE)).expect("the cube fixture reads");
    project.models[0].source = Arc::new(cuboid(Vec3::ZERO, Vec3::splat(side_mm)));
    project.models[0].shell = None;
    let object = &mut project.manifest.objects[0];
    object.transform = Transform::default();
    object.hollow.built = None;
    project
}

/// The top face alone of a cube `side_mm` on a side standing at the origin: a sheet that
/// encloses nothing.
pub fn sheet_of(side_mm: f32) -> Mesh {
    let mut open = cuboid(Vec3::ZERO, Vec3::splat(side_mm));
    let vertices = open.vertices.clone();
    open.faces.retain(|face| {
        face.iter()
            .all(|at| vertices[*at as usize].z > side_mm / 2.0)
    });
    open
}

/// [`cube`]'s plate round [`sheet_of`] its side.
pub fn sheet() -> Project {
    let mut project = cube();
    project.models[0].source = Arc::new(sheet_of(SIDE_MM));
    project
}

/// [`cube`] twice, ids 1 and 2, and a third object, id 3, which is a plug for the second
/// cube, named by its place among the objects.
pub fn cubes_with_plug() -> Project {
    let mut project = cube();
    let meshes = project.models[0].clone();
    for id in [2, 3] {
        let state = &project.manifest.objects[0];
        let mut copy = core_engine::project::ObjectState {
            id: ObjectId(id),
            ..state.clone()
        };
        copy.name = format!("cube {id}");
        project.manifest.objects.push(copy);
        project.models.push(meshes.clone());
    }
    project.manifest.objects[0].id = ObjectId(1);
    project.manifest.objects[2].plug_of = Some(1);
    project
}

/// [`cube`] with the winding of face `face` of its mesh turned, so it is no longer a solid.
pub fn cube_with_turned_face(face: usize) -> Project {
    let mut project = cube();
    let mut mesh = (*project.models[0].source).clone();
    mesh.faces[face].swap(1, 2);
    project.models[0].source = Arc::new(mesh);
    project
}

/// [`cube`] with face `face` of its mesh taken out, leaving a hole.
pub fn cube_with_hole(face: usize) -> Project {
    let mut project = cube();
    let mut mesh = (*project.models[0].source).clone();
    mesh.faces.remove(face);
    project.models[0].source = Arc::new(mesh);
    project
}
