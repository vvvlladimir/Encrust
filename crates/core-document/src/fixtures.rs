//! The plates the tests edit.

use std::path::Path;
use std::sync::Arc;

use core_engine::project::{Project, load};
use core_geometry::{Transform, Vec3, cuboid};

/// A project of version 4, before ids, holding one model named "cube".
const CUBE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../encrust-cli/tests/fixtures/cube.encrust"
);

/// The side of [`cube`]'s cube, millimetres.
pub const SIDE_MM: f32 = 10.0;

/// A plate of one solid cube named "cube", [`SIDE_MM`] on a side, standing on the plate at
/// the origin: the project file's manifest round a body of known volume.
pub fn cube() -> Project {
    let mut project = load(Path::new(CUBE)).expect("the cube fixture reads");
    project.models[0].source = Arc::new(cuboid(Vec3::ZERO, Vec3::splat(SIDE_MM)));
    project.models[0].shell = None;
    let object = &mut project.manifest.objects[0];
    object.transform = Transform::default();
    object.hollow.built = None;
    project
}
