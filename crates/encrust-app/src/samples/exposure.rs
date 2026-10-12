//! The exposure check: a plaque with four rows of features from fine to coarse. A layer
//! underexposed loses the thinnest pins and fins; one overexposed closes the narrowest slots
//! and holes.

use core_geometry::{Mesh, Vec2, Vec3};

use crate::samples::mark;
use crate::samples::solid::{EMBED, Solid, holed_rectangle, rod};

/// The plaque, mm.
const BASE: Vec3 = Vec3::new(46.0, 30.0, 2.0);
/// How tall every feature stands over the plaque, mm.
const FEATURE_H: f32 = 4.0;
/// Pin diameters, fin widths, slot widths and hole diameters, column by column, mm.
const PINS_MM: [f32; 6] = [0.3, 0.4, 0.5, 0.6, 0.8, 1.0];
const FINS_MM: [f32; 6] = [0.2, 0.3, 0.4, 0.5, 0.6, 0.8];
const SLOTS_MM: [f32; 6] = [0.1, 0.15, 0.2, 0.3, 0.4, 0.5];
const HOLES_MM: [f32; 6] = [0.4, 0.5, 0.6, 0.8, 1.0, 1.2];
/// Where the first column stands and how far apart the columns are, mm.
const FIRST_X: f32 = 4.0;
const PITCH: f32 = 5.0;
/// Half the width of a block either side of a slot and of a tower round a hole, mm.
const BLOCK_HALF: f32 = 1.6;
const SEGMENTS: u32 = 32;

pub fn build() -> Mesh {
    let mut solid = Solid::default();
    solid.cuboid(Vec3::ZERO, BASE);
    let (floor, top) = (BASE.z - EMBED, BASE.z + FEATURE_H);
    for column in 0..PINS_MM.len() {
        let x = FIRST_X + PITCH * column as f32;
        let pin = rod(PINS_MM[column] / 2.0, floor, top);
        solid.revolve(Vec2::new(x, 4.5), &pin, SEGMENTS);
        let fin = FINS_MM[column] / 2.0;
        solid.cuboid(
            Vec3::new(x - fin, 8.5, floor),
            Vec3::new(x + fin, 12.5, top),
        );
        let slot = SLOTS_MM[column] / 2.0;
        let (near, far) = (slot, slot + BLOCK_HALF);
        solid.cuboid(
            Vec3::new(x - far, 15.0, floor),
            Vec3::new(x - near, 19.0, top),
        );
        solid.cuboid(
            Vec3::new(x + near, 15.0, floor),
            Vec3::new(x + far, 19.0, top),
        );
        let centre = Vec2::new(x, 24.0);
        let (outer, inner) = holed_rectangle(
            centre - Vec2::splat(BLOCK_HALF),
            centre + Vec2::splat(BLOCK_HALF),
            centre,
            HOLES_MM[column] / 2.0,
            SEGMENTS,
        );
        solid.tube(&outer, &inner, floor, top);
    }
    mark::stand(&mut solid, Vec2::new(40.0, 15.0), 8.0, 4.5, floor);
    solid.into_mesh()
}
