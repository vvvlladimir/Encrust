//! The fit gauge: a block with a hole of a known size and loose pins from just under it to
//! just over. Which pins go in tells how far the resin closes a hole and opens a wall.

use core_geometry::{Mesh, Vec2, Vec3};

use crate::samples::mark;
use crate::samples::solid::{EMBED, Solid, holed_rectangle};

/// The hole's diameter, and how much each pin is under or over it, smallest first, mm.
pub const HOLE_MM: f32 = 6.0;
pub const CLEARANCES_MM: [f32; 7] = [-0.30, -0.20, -0.15, -0.10, -0.05, 0.0, 0.10];
/// The block, mm, and where the hole goes through it.
const BLOCK: Vec3 = Vec3::new(26.0, 14.0, 5.0);
const HOLE_AT: Vec2 = Vec2::new(7.0, 7.0);
/// The shortest pin, mm; each next one stands a millimetre taller so the row can be told apart.
const PIN_H: f32 = 7.0;
pub const PIN_PITCH: f32 = 8.0;
/// The chamfer at a pin's foot, against the first layers spreading, and at its tip, to lead
/// it into the hole, mm.
const FOOT_CHAMFER: f32 = 0.4;
const TIP_CHAMFER: f32 = 0.5;
/// Fine enough that the facets miss a true circle by under 2 µm.
const SEGMENTS: u32 = 96;

pub fn build() -> Mesh {
    let mut solid = Solid::default();
    let (outer, inner) = holed_rectangle(
        Vec2::ZERO,
        BLOCK.truncate(),
        HOLE_AT,
        HOLE_MM / 2.0,
        SEGMENTS,
    );
    solid.tube(&outer, &inner, 0.0, BLOCK.z);
    mark::stand(
        &mut solid,
        Vec2::new(18.5, BLOCK.y / 2.0),
        8.0,
        4.0,
        BLOCK.z - EMBED,
    );
    for (index, clearance) in CLEARANCES_MM.iter().enumerate() {
        let radius = (HOLE_MM + clearance) / 2.0;
        let height = PIN_H + index as f32;
        let centre = Vec2::new(4.0 + PIN_PITCH * index as f32, -7.0);
        solid.revolve(centre, &pin(radius, height), SEGMENTS);
    }
    solid.into_mesh()
}

/// A pin's profile: chamfered at its foot and its tip.
fn pin(radius: f32, height: f32) -> [Vec2; 6] {
    [
        Vec2::new(0.0, 0.0),
        Vec2::new(radius - FOOT_CHAMFER, 0.0),
        Vec2::new(radius, FOOT_CHAMFER),
        Vec2::new(radius, height - TIP_CHAMFER),
        Vec2::new(radius - TIP_CHAMFER, height),
        Vec2::new(0.0, height),
    ]
}
