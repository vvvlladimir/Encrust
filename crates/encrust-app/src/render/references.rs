//! Things everyone knows the size of, laid beside the plate in the render view so a model's
//! printed size reads at a glance. See docs/design/viewport.md.

use std::sync::{Arc, OnceLock};

use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3, ball, cuboid, cylinder};

use crate::plate::BuildPlate;
use crate::render::gpu::ModelDraw;
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// An ID-1 card (ISO/IEC 7810), millimetres: across, along and thick.
const CARD_MM: [Scalar; 3] = [53.98, 85.6, 0.76];
/// A one-euro coin, millimetres: diameter and thickness.
const COIN_MM: [Scalar; 2] = [23.25, 2.33];
/// The one-inch round base a tabletop miniature of a person stands on, millimetres.
const BASE_MM: [Scalar; 2] = [25.0, 3.0];
/// A "32 mm heroic" figure: millimetres from its soles to its eyes, which is what the scale
/// names, not to the top of its head.
const EYE_LEVEL_MM: Scalar = 32.0;
/// A ruler, millimetres: long, wide and thick, ticked every 5 mm and long at every 10.
const RULER_MM: [Scalar; 3] = [100.0, 12.0, 1.2];
const TICK_MM: Scalar = 5.0;
/// Half a tick's width, and how far it stands proud of the ruler, millimetres.
const TICK_HALF_MM: Scalar = 0.25;
const TICK_PROUD_MM: Scalar = 0.4;
/// Millimetres between the plate's edge and the first lane, and between references.
const GAP_MM: Scalar = 8.0;
/// Flats round a coin or a base: round to the eye at any zoom the plate is framed at.
const ROUND_SEGMENTS: usize = 64;

/// One reference: its mesh, footprint centred on its own origin and standing on z = 0, and
/// its footprint, millimetres across and along.
struct Reference {
    mesh: Arc<Mesh>,
    footprint: Vec2,
}

fn disc([diameter, height]: [Scalar; 2]) -> Mesh {
    cylinder(Vec3::ZERO, Vec3::Z * height, diameter / 2.0, ROUND_SEGMENTS)
}

fn card() -> Reference {
    let [across, along, thick] = CARD_MM;
    let half = Vec3::new(across, along, 0.0) / 2.0;
    Reference {
        mesh: Arc::new(cuboid(-half, half + Vec3::Z * thick)),
        footprint: Vec2::new(across, along),
    }
}

fn coin() -> Reference {
    Reference {
        mesh: Arc::new(disc(COIN_MM)),
        footprint: Vec2::splat(COIN_MM[0]),
    }
}

/// A person at miniature scale on its base, in heroic proportions: a head a sixth of its
/// eye height, limbs as bars. Recognisable, not a sculpt.
fn figure() -> Reference {
    let mut mesh = disc(BASE_MM);
    let at = |x: Scalar, z: Scalar| Vec3::new(x, 0.0, BASE_MM[1] + z);
    let head_mm = EYE_LEVEL_MM / 6.0;
    let shoulder = EYE_LEVEL_MM - head_mm;
    let hip = EYE_LEVEL_MM * 0.47;
    let parts = [
        cylinder(at(-1.8, 0.0), at(-1.6, hip), 1.6, 12),
        cylinder(at(1.8, 0.0), at(1.6, hip), 1.6, 12),
        cylinder(at(0.0, hip - 1.0), at(0.0, shoulder), 3.6, 16),
        cylinder(at(-4.6, shoulder - 0.6), at(-5.6, hip - 0.5), 1.2, 10),
        cylinder(at(4.6, shoulder - 0.6), at(5.6, hip - 0.5), 1.2, 10),
        ball(at(0.0, EYE_LEVEL_MM), head_mm / 2.0, 12, 16),
    ];
    for part in &parts {
        mesh.append(part);
    }
    Reference {
        mesh: Arc::new(mesh),
        footprint: Vec2::splat(BASE_MM[0]),
    }
}

/// A ruler lying along the plate's depth, its ticks standing proud of it.
fn ruler() -> Reference {
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
    Reference {
        mesh: Arc::new(mesh),
        footprint: Vec2::new(wide, long),
    }
}

/// Built once: the GPU cache keys a mesh by its address, so the same `Arc` every frame. Each
/// lane runs from the plate's front back, the first nearest the plate.
fn lanes() -> &'static [Vec<Reference>] {
    static BUILT: OnceLock<Vec<Vec<Reference>>> = OnceLock::new();
    BUILT.get_or_init(|| vec![vec![ruler()], vec![card(), coin(), figure()]])
}

/// The references with where each stands: lanes along the plate's side to the viewer's
/// right, from its front edge back, turned with the plate's front.
pub fn placed(plate: &BuildPlate) -> Vec<(Arc<Mesh>, Transform)> {
    let turn = plate.front_turn();
    let (rightward, away) = (turn * Vec3::X, turn * Vec3::Y);
    let extent = |axis: Vec3| axis.x.abs() * plate.x_mm + axis.y.abs() * plate.y_mm;
    let mut side = extent(rightward) / 2.0 + GAP_MM;
    let mut placed = Vec::new();
    for lane in lanes() {
        let mut along = -extent(away) / 2.0;
        for reference in lane {
            let centre = plate.center()
                + rightward * (side + reference.footprint.x / 2.0)
                + away * (along + reference.footprint.y / 2.0);
            along += reference.footprint.y + GAP_MM;
            let transform = Transform::from_scale_rotation_translation(Vec3::ONE, turn, centre);
            placed.push((Arc::clone(&reference.mesh), transform));
        }
        let widest = lane.iter().map(|reference| reference.footprint.x);
        side += widest.fold(0.0, Scalar::max) + GAP_MM;
    }
    placed
}

/// The references as markers in their own token: drawn, never picked, sliced or capped.
pub fn drawn(plate: &BuildPlate) -> impl Iterator<Item = ModelDraw> {
    placed(plate).into_iter().map(|(mesh, at)| {
        ModelDraw::whole(
            mesh,
            ModelInstance::new(at, theme::scene().reference, NOT_MARKED),
        )
    })
}

#[cfg(test)]
mod tests;
