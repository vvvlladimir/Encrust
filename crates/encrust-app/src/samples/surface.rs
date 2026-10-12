//! The surface finish test: a dome and a round column show the steps of the pixel grid and
//! how far anti-aliasing smooths them, and a shallow ramp shows the layers one by one.

use core_geometry::{Mesh, Vec2, Vec3};

use crate::samples::mark;
use crate::samples::solid::{EMBED, Solid, rod};

/// The base, mm.
const BASE: Vec3 = Vec3::new(46.0, 24.0, 1.5);
const DOME_R: f32 = 7.0;
const COLUMN_R: f32 = 4.5;
const COLUMN_H: f32 = 12.0;
/// The ramp's run along X, mm, and its slope, degrees: shallow, so each layer is a band.
const RAMP_RUN: f32 = 15.0;
const RAMP_SLOPE: f32 = 5.0;
/// Fine, so the facets stay under a pixel and what shows is the printer's own steps.
const SEGMENTS: u32 = 128;
const DOME_RINGS: u32 = 32;

pub fn build() -> Mesh {
    let mut solid = Solid::default();
    solid.cuboid(Vec3::ZERO, BASE);
    let floor = BASE.z - EMBED;
    solid.revolve(Vec2::new(9.0, 12.0), &dome(floor), SEGMENTS);
    let column = rod(COLUMN_R, floor, BASE.z + COLUMN_H);
    solid.revolve(Vec2::new(23.0, 12.0), &column, SEGMENTS);
    let rise = RAMP_RUN * RAMP_SLOPE.to_radians().tan();
    let ramp = [
        Vec2::new(29.5, floor),
        Vec2::new(29.5 + RAMP_RUN, floor),
        Vec2::new(29.5 + RAMP_RUN, BASE.z + rise),
        Vec2::new(29.5, BASE.z),
    ];
    solid.side_prism(&ramp, 3.0, 11.0);
    mark::stand(&mut solid, Vec2::new(37.0, 17.5), 7.0, 4.2, floor);
    solid.into_mesh()
}

/// A half sphere's profile standing on the base, sunk into it by `EMBED`.
fn dome(floor: f32) -> Vec<Vec2> {
    let mut profile = vec![Vec2::new(0.0, floor), Vec2::new(DOME_R, floor)];
    profile.extend((0..=DOME_RINGS).map(|ring| {
        let angle = std::f32::consts::FRAC_PI_2 * ring as f32 / DOME_RINGS as f32;
        let (sin, cos) = angle.sin_cos();
        Vec2::new(DOME_R * cos, BASE.z + DOME_R * sin)
    }));
    profile
}
