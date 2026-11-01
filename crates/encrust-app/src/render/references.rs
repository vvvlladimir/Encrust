//! Things everyone knows the size of, laid beside the plate in the render view so a model's
//! printed size reads at a glance: a bank card, a coin, a miniature's base.

use std::sync::{Arc, OnceLock};

use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3, cuboid, cylinder};

use crate::plate::BuildPlate;
use crate::render::gpu::ModelDraw;
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// An ID-1 card (ISO/IEC 7810), millimetres: across, along and thick.
const CARD_MM: [Scalar; 3] = [53.98, 85.6, 0.76];
/// A one-euro coin, millimetres: diameter and thickness.
const COIN_MM: [Scalar; 2] = [23.25, 2.33];
/// The round base most tabletop miniatures stand on, millimetres: diameter and height.
const BASE_MM: [Scalar; 2] = [32.0, 3.0];
/// Millimetres between the plate's edge and the first reference, and between references.
const GAP_MM: Scalar = 8.0;
/// Flats round a coin or a base: round to the eye at any zoom the plate is framed at.
const ROUND_SEGMENTS: usize = 64;

/// One reference: its mesh, footprint centred on its own origin and standing on z = 0, and
/// its footprint, millimetres across and along.
struct Reference {
    mesh: Arc<Mesh>,
    footprint: Vec2,
}

fn disc([diameter, height]: [Scalar; 2]) -> Reference {
    Reference {
        mesh: Arc::new(cylinder(
            Vec3::ZERO,
            Vec3::Z * height,
            diameter / 2.0,
            ROUND_SEGMENTS,
        )),
        footprint: Vec2::splat(diameter),
    }
}

/// Built once: the GPU cache keys a mesh by its address, so the same `Arc` every frame.
fn references() -> &'static [Reference] {
    static BUILT: OnceLock<Vec<Reference>> = OnceLock::new();
    BUILT.get_or_init(|| {
        let [across, along, thick] = CARD_MM;
        let half = Vec3::new(across, along, 0.0) / 2.0;
        let card = Reference {
            mesh: Arc::new(cuboid(-half, half + Vec3::Z * thick)),
            footprint: Vec2::new(across, along),
        };
        vec![card, disc(COIN_MM), disc(BASE_MM)]
    })
}

/// The references with where each stands: a column along the plate's side to the viewer's
/// right, from its front edge back, turned with the plate's front.
pub fn placed(plate: &BuildPlate) -> Vec<(Arc<Mesh>, Transform)> {
    let turn = plate.front_turn();
    let (rightward, away) = (turn * Vec3::X, turn * Vec3::Y);
    let extent = |axis: Vec3| axis.x.abs() * plate.x_mm + axis.y.abs() * plate.y_mm;
    let side = plate.center() + rightward * (extent(rightward) / 2.0 + GAP_MM);
    let mut along = -extent(away) / 2.0;
    references()
        .iter()
        .map(|reference| {
            let centre = side
                + rightward * reference.footprint.x / 2.0
                + away * (along + reference.footprint.y / 2.0);
            along += reference.footprint.y + GAP_MM;
            let transform = Transform::from_scale_rotation_translation(Vec3::ONE, turn, centre);
            (Arc::clone(&reference.mesh), transform)
        })
        .collect()
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
mod tests {
    use core_geometry::transform_mesh;

    use super::*;

    /// Millimetres a placed reference may miss its closed-form size by: the round flats.
    const TOLERANCE_MM: Scalar = 0.05;

    fn bounds_of(plate: &BuildPlate) -> Vec<core_geometry::Aabb> {
        placed(plate)
            .iter()
            .map(|(mesh, transform)| {
                transform_mesh(mesh, *transform)
                    .aabb()
                    .expect("a reference is never empty")
            })
            .collect()
    }

    #[test]
    fn references_keep_their_real_size() {
        let sizes: Vec<Vec3> = bounds_of(&BuildPlate::default())
            .iter()
            .map(|aabb| aabb.maxs - aabb.mins)
            .collect();
        let expected = [
            Vec3::from(CARD_MM),
            Vec3::new(COIN_MM[0], COIN_MM[0], COIN_MM[1]),
            Vec3::new(BASE_MM[0], BASE_MM[0], BASE_MM[1]),
        ];
        for (size, expected) in sizes.iter().zip(expected) {
            assert!(
                (*size - expected).abs().max_element() < TOLERANCE_MM,
                "expected {expected} mm, got {size} mm"
            );
        }
    }

    #[test]
    fn references_stand_on_the_floor_beside_the_plate_without_touching() {
        let plate = BuildPlate::default();
        let boxes = bounds_of(&plate);
        for aabb in &boxes {
            assert!(
                aabb.mins.z.abs() < 1e-4,
                "expected to stand on z = 0, got {}",
                aabb.mins.z
            );
            assert!(
                aabb.mins.x >= plate.x_mm,
                "expected right of x = {} mm",
                plate.x_mm
            );
        }
        for (index, low) in boxes.iter().enumerate() {
            for high in &boxes[index + 1..] {
                assert!(
                    high.mins.y >= low.maxs.y,
                    "expected references in a row, apart"
                );
            }
        }
    }
}
