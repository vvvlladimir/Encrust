//! The detail test, a first print that needs no supports: the mark standing large, its
//! slabs overhanging the necks between them, a field of spikes whose tips narrow one by one,
//! and a pillar with ledges overhanging at three angles.

use core_geometry::{Mesh, Vec2, Vec3};

use crate::samples::mark;
use crate::samples::solid::{EMBED, Solid};

/// The plinth, mm.
const BASE: Vec3 = Vec3::new(46.0, 24.0, 2.5);
/// The spikes' rows and columns, their foot radius and height, and the radius of the
/// broadest tip and how much each next one narrows, mm.
const SPIKE_ROWS: usize = 4;
const SPIKE_COLUMNS: usize = 3;
const SPIKE_FOOT: f32 = 1.2;
const SPIKE_H: f32 = 7.0;
const BROADEST_TIP: f32 = 0.6;
const TIP_STEP: f32 = 0.05;
/// The ledges' angles over the horizontal, degrees, and how far each reaches out, mm.
const LEDGE_ANGLES: [f32; 3] = [20.0, 30.0, 45.0];
const LEDGE_REACH: f32 = 5.0;
/// The pillar the ledges hang from: its x, and its height over the plinth, mm.
const PILLAR_X: [f32; 2] = [33.0, 35.0];
const PILLAR_H: f32 = 11.0;
const SEGMENTS: u32 = 24;

pub fn build() -> Mesh {
    let mut solid = Solid::default();
    solid.cuboid(Vec3::ZERO, BASE);
    let floor = BASE.z - EMBED;
    mark::stand(&mut solid, Vec2::new(10.0, 12.0), 16.0, 15.0, floor);
    spikes(&mut solid, floor);
    ledges(&mut solid, floor);
    solid.into_mesh()
}

/// Cones in a grid, each tip `TIP_STEP` narrower than the last.
fn spikes(solid: &mut Solid, floor: f32) {
    for index in 0..SPIKE_ROWS * SPIKE_COLUMNS {
        let (row, column) = (index / SPIKE_COLUMNS, index % SPIKE_COLUMNS);
        let centre = Vec2::new(21.0 + 3.5 * column as f32, 4.5 + 5.0 * row as f32);
        let tip = BROADEST_TIP - TIP_STEP * index as f32;
        let profile = [
            Vec2::new(0.0, floor),
            Vec2::new(SPIKE_FOOT, floor),
            Vec2::new(tip, BASE.z + SPIKE_H),
            Vec2::new(0.0, BASE.z + SPIKE_H),
        ];
        solid.revolve(centre, &profile, SEGMENTS);
    }
}

/// A pillar with one ledge per angle along its side, their tops level.
fn ledges(solid: &mut Solid, floor: f32) {
    let top = BASE.z + PILLAR_H;
    let [near, far] = PILLAR_X;
    solid.cuboid(Vec3::new(near, 3.0, floor), Vec3::new(far, 21.0, top));
    for (band, angle) in LEDGE_ANGLES.iter().enumerate() {
        let drop = LEDGE_REACH * angle.to_radians().tan();
        let profile = [
            Vec2::new(far - EMBED, top - 1.0 - drop),
            Vec2::new(far + LEDGE_REACH, top - 1.0),
            Vec2::new(far - EMBED, top - 1.0),
        ];
        let y = 3.0 + 6.5 * band as f32;
        solid.side_prism(&profile, y, y + 5.0);
    }
}
