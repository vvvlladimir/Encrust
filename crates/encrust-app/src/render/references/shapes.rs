//! The meshes on the desk, each in its own frame, standing on z = 0, at its real size.

use std::f32::consts::PI;

use core_geometry::{
    Mesh, Scalar, Vec2, Vec3, cuboid, cylinder, prism, revolve, signed_volume, turns,
};

/// A one-euro coin, millimetres: diameter and thickness.
pub const COIN_MM: [Scalar; 2] = [23.25, 2.33];
/// A 330 ml drinks can, millimetres: its radius and height.
pub const CAN_RADIUS_MM: Scalar = 33.1;
pub const CAN_HEIGHT_MM: Scalar = 115.2;
/// Millimetres the lid sits under the rim.
const LID_SUNK_MM: Scalar = 2.0;
/// A ballpoint pen as long as a BIC Cristal, capped, and the radius of its cap, which it
/// lies on: millimetres.
pub const PEN_MM: Scalar = 149.5;
pub const PEN_REST_MM: Scalar = 4.4;
/// A ruler, millimetres: long, wide and thick, ticked every 5 mm and long at every 10.
pub const RULER_MM: [Scalar; 3] = [100.0, 12.0, 1.2];
const TICK_MM: Scalar = 5.0;
const TICK_HALF_MM: Scalar = 0.25;
const TICK_PROUD_MM: Scalar = 0.3;
/// A banana: the radius of the arc its spine follows and the angles it runs between, and
/// its greatest radius across. Must match `BANANA_FROM` and `BANANA_TO` in shader.wgsl.
pub const BANANA_ARC_MM: Scalar = 110.0;
pub const BANANA_FROM: Scalar = 0.75;
pub const BANANA_TO: Scalar = 2.39;
const BANANA_GIRTH_MM: Scalar = 17.5;
/// Where along it the stem starts, its radius, and the least radius of its blossom end.
const STEM_FROM: Scalar = 0.9;
const STEM_MM: Scalar = 4.0;
const BLOSSOM_MM: Scalar = 3.0;
/// Rings along the banana and points round each, its five ridges as a ripple on the round.
const BANANA_RINGS: usize = 72;
const BANANA_ROUND: usize = 20;
const RIDGES: Scalar = 5.0;
const RIDGE_DEPTH: Scalar = 0.06;
/// Flats round anything turned: round to the eye at any zoom the plate is framed at.
const ROUND: usize = 96;

/// A coin with its raised rim: a thinner field inside a ring as thick as the coin.
pub fn coin() -> Mesh {
    let [diameter, thick] = COIN_MM;
    let edge = diameter / 2.0;
    let rings = [
        (0.0, edge),
        (thick, edge),
        (thick, edge - 0.9),
        (thick - 0.3, edge - 1.2),
    ];
    let round: Vec<Vec2> = turns(ROUND).collect();
    revolve(Vec3::ZERO, Vec3::Z, &rings, &round)
}

/// A drinks can: a narrower foot and a necked-in neck round a straight wall, its lid sunk
/// inside a rolled rim, and the tab that opens it lying on the lid.
pub fn can() -> Mesh {
    let (wall, height) = (CAN_RADIUS_MM, CAN_HEIGHT_MM);
    let lid = height - LID_SUNK_MM;
    let rings = [
        (0.0, 26.0),
        (1.5, 30.5),
        (5.0, wall),
        (height - 11.0, wall),
        (height - 5.5, 29.5),
        (height - 0.6, 27.3),
        (height, 26.6),
        (height - 0.4, 25.6),
        (lid, 25.0),
    ];
    let round: Vec<Vec2> = turns(ROUND).collect();
    let mut mesh = revolve(Vec3::ZERO, Vec3::Z, &rings, &round);
    mesh.append(&cuboid(
        Vec3::new(-6.0, -2.0, lid),
        Vec3::new(6.0, 17.0, lid + 0.6),
    ));
    mesh.append(&cylinder(
        Vec3::Z * lid,
        Vec3::Z * (lid + 1.0),
        1.6,
        ROUND / 6,
    ));
    mesh
}

/// A ballpoint pen along +z from its tip: a cone, a hexagonal barrel, a round cap and its
/// clip. Lies on its cap, the clip up once turned onto the mat.
pub fn pen() -> Mesh {
    let point: Vec<Vec2> = turns(ROUND / 4).collect();
    let mut mesh = revolve(Vec3::ZERO, Vec3::Z, &[(0.0, 0.4), (14.0, 3.0)], &point);
    let across_corners = 3.75 / (PI / 6.0).cos();
    let hexagon: Vec<Vec2> = turns(6)
        .map(|corner| Vec2::from_angle(PI / 6.0).rotate(corner) * across_corners)
        .collect();
    mesh.append(&prism(&hexagon, 13.0, 128.0));
    mesh.append(&cylinder(
        Vec3::Z * 124.0,
        Vec3::Z * PEN_MM,
        PEN_REST_MM,
        ROUND / 2,
    ));
    mesh.append(&cuboid(
        Vec3::new(-1.2, -PEN_REST_MM - 1.0, 128.0),
        Vec3::new(1.2, -PEN_REST_MM + 0.4, 146.0),
    ));
    mesh
}

/// A ruler along y, centred on its own origin, its ticks standing proud along one edge.
pub fn ruler() -> Mesh {
    let [long, wide, thick] = RULER_MM;
    let edge = -wide / 2.0;
    let mut mesh = cuboid(
        Vec3::new(edge, -long / 2.0, 0.0),
        Vec3::new(-edge, long / 2.0, thick),
    );
    let ticks = (long / TICK_MM).round() as usize;
    for tick in 0..=ticks {
        // The end ticks stand inside the ruler, so it stays exactly its length.
        let y = (-long / 2.0 + tick as Scalar * TICK_MM - TICK_HALF_MM)
            .clamp(-long / 2.0, long / 2.0 - 2.0 * TICK_HALF_MM);
        let reach = if tick % 2 == 0 {
            wide / 2.0
        } else {
            wide / 4.0
        };
        mesh.append(&cuboid(
            Vec3::new(edge, y, thick),
            Vec3::new(edge + reach, y + 2.0 * TICK_HALF_MM, thick + TICK_PROUD_MM),
        ));
    }
    mesh
}

/// The banana's radius across at `share` of its length from the blossom end to the stem.
fn banana_radius(share: Scalar) -> Scalar {
    if share >= STEM_FROM {
        return STEM_MM;
    }
    let body = (PI * share / STEM_FROM).sin().max(0.0).sqrt();
    (BANANA_GIRTH_MM * body).max(BLOSSOM_MM)
}

/// The middle of the banana at `share` of its length, lying on its side on z = 0: its
/// spine on an arc about the origin, raised by its radius.
pub fn banana_spine(share: Scalar) -> Vec3 {
    let angle = BANANA_FROM + share * (BANANA_TO - BANANA_FROM);
    let radius = banana_radius(share);
    (Vec2::from_angle(angle) * BANANA_ARC_MM).extend(radius)
}

/// A banana lying on its side: rings swept along its spine, each its ridged round at the
/// radius there, closed at both ends.
pub fn banana() -> Mesh {
    let mut vertices = Vec::with_capacity((BANANA_RINGS + 1) * BANANA_ROUND + 2);
    for ring in 0..=BANANA_RINGS {
        let share = ring as Scalar / BANANA_RINGS as Scalar;
        let spine = banana_spine(share);
        let outward = spine.truncate().normalize().extend(0.0);
        for step in turns(BANANA_ROUND) {
            let ridge = 1.0 + RIDGE_DEPTH * (RIDGES * step.to_angle()).cos();
            let across = (outward * step.x + Vec3::Z * step.y) * banana_radius(share) * ridge;
            vertices.push(spine + across);
        }
    }
    let (blossom, stem) = (vertices.len() as u32, vertices.len() as u32 + 1);
    vertices.push(banana_spine(0.0));
    vertices.push(banana_spine(1.0));
    let round = BANANA_ROUND as u32;
    let mut faces = Vec::with_capacity(2 * (BANANA_RINGS + 1) * BANANA_ROUND);
    for ring in 0..BANANA_RINGS as u32 {
        let (this, next) = (ring * round, (ring + 1) * round);
        for step in 0..round {
            let after = (step + 1) % round;
            faces.push([this + step, next + step, next + after]);
            faces.push([this + step, next + after, this + after]);
        }
    }
    let last = BANANA_RINGS as u32 * round;
    for step in 0..round {
        let after = (step + 1) % round;
        faces.push([blossom, step, after]);
        faces.push([stem, last + after, last + step]);
    }
    // The ridges reach under the spine's own radius here and there: it rests on the lowest.
    let lowest = vertices
        .iter()
        .map(|at| at.z)
        .fold(Scalar::MAX, Scalar::min);
    vertices.iter_mut().for_each(|at| at.z -= lowest);
    let mut mesh = Mesh::new(vertices, faces);
    if signed_volume(&mesh) < 0.0 {
        mesh.faces.iter_mut().for_each(|face| face.swap(1, 2));
    }
    mesh
}
