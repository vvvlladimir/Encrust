use core_geometry::{Mesh, Quat, Scalar, Transform, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, Winding};
use rayon::prelude::*;

use crate::candidates::directions;
use crate::error::PlateError;
use crate::score::{Faces, Score};

/// What the search is allowed to spend, and what counts as an overhang while it looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientSettings {
    /// How far a surface may lean before it needs holding up, degrees from vertical, the
    /// same measure a support profile carries.
    pub max_overhang_deg: Scalar,
    /// Directions taken off the sphere, on top of the model's own flat faces.
    pub samples: usize,
    /// Candidates carried into the second pass, where each one is sliced to find the
    /// cross-section the peel has to pull.
    pub shortlist: usize,
    /// How many planes a shortlisted candidate is cut at. These are samples across the
    /// model's height, not the layers it will print at.
    pub sections: usize,
}

impl Default for OrientSettings {
    fn default() -> Self {
        Self {
            max_overhang_deg: 45.0,
            samples: 200,
            shortlist: 8,
            sections: 24,
        }
    }
}

/// How a model should stand, and what that costs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oriented {
    /// The rotation to place the model with. It replaces whatever rotation the model
    /// carries: the search measures the mesh in its own space.
    pub rotation: Quat,
    pub score: Score,
    /// Orientations that were measured, after the ones a print cannot tell apart were
    /// dropped.
    pub considered: usize,
}

/// Finds the orientation a model prints best in.
///
/// Two passes, because the terms cost two different things: every candidate is scored on
/// its overhangs, its footprint and its height, which are read off the mesh without
/// turning it, and only the best few are cut to find the cross-section the peel has to
/// pull. See `docs/decisions/0087`.
pub fn orient(mesh: &Mesh, settings: &OrientSettings) -> Result<Oriented, PlateError> {
    let faces = Faces::of(mesh).ok_or(PlateError::NothingToOrient)?;
    let candidates = directions(mesh, settings.samples);
    if candidates.is_empty() {
        return Err(PlateError::NothingToOrient);
    }

    let mut ranked: Vec<(Vec3, Score)> = candidates
        .par_iter()
        .map(|down| (*down, faces.score(*down, settings.max_overhang_deg)))
        .collect();
    ranked.sort_by(|a, b| a.1.total.total_cmp(&b.1.total));

    let best = ranked
        .par_iter()
        .take(settings.shortlist.max(1))
        .map(|(down, score)| {
            let peak = peak_section_mm2(mesh, *down, settings.sections)?;
            Ok((*down, faces.with_peak(*score, peak)))
        })
        .collect::<Result<Vec<_>, PlateError>>()?
        .into_iter()
        .min_by(|a, b| a.1.total.total_cmp(&b.1.total))
        .ok_or(PlateError::NothingToOrient)?;

    Ok(Oriented {
        rotation: rotation_for(best.0),
        score: best.1,
        considered: ranked.len(),
    })
}

/// The rotation that stands a model up with `down` pointing at the plate.
///
/// Which way it faces about the vertical is left to the shortest turn: the plate is
/// round to a print, and arranging is what decides where a model looks.
pub fn rotation_for(down: Vec3) -> Quat {
    down.try_normalize().map_or(Quat::IDENTITY, |unit| {
        Quat::from_rotation_arc(unit, Vec3::NEG_Z)
    })
}

/// The largest cross-section the model has when it stands with `down` at the plate,
/// square millimetres.
///
/// This is the peel force the film sees, which is what an MSLA print fails on before it
/// fails on a support; the model is cut at `sections` heights rather than at every layer,
/// because the largest section is a property of the shape and not of the layer height.
fn peak_section_mm2(mesh: &Mesh, down: Vec3, sections: usize) -> Result<Scalar, PlateError> {
    let placed = transform_mesh(
        mesh,
        Transform {
            rotation: rotation_for(down),
            ..Transform::default()
        },
    );
    let bounds = placed.aabb().ok_or(PlateError::NothingToOrient)?;

    let sections = sections.max(1);
    let span = bounds.maxs.z - bounds.mins.z;
    if span <= Scalar::EPSILON {
        return Ok(0.0);
    }
    let heights: Vec<Scalar> = (0..sections)
        .map(|index| bounds.mins.z + span * (index as Scalar + 0.5) / sections as Scalar)
        .collect();

    let sliced = PlaneSliceEngine.slice_at(&placed, &heights)?;
    Ok(sliced
        .layers
        .iter()
        .map(|layer| {
            layer
                .contours
                .iter()
                .map(|contour| match contour.winding {
                    Winding::Outer => contour.area(),
                    Winding::Inner => -contour.area(),
                })
                .sum::<Scalar>()
                .max(0.0)
        })
        .fold(0.0, Scalar::max))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    /// Where the model's own `axis` ends up once the orientation is applied.
    fn placed(rotation: Quat, axis: Vec3) -> Vec3 {
        rotation * axis
    }

    #[test]
    fn a_slab_is_stood_on_its_edge_rather_than_laid_flat() {
        let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a slab can be oriented");

        // Lying flat, every layer would peel a 1600 mm2 section off the film. The thin
        // axis therefore ends up across the plate rather than up it.
        let thin = placed(found.rotation, Vec3::Z);
        assert!(
            thin.z.abs() < 0.05,
            "the wide faces end up vertical, got {thin}"
        );
        let peak = found.score.peak_mm2.expect("the winner was sliced");
        assert!(peak < 200.0, "a 40 by 4 section, got {peak}");
        assert!(found.considered > 100, "the sphere was searched too");
    }

    #[test]
    fn a_tall_column_prints_standing_up() {
        let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a column can be oriented");

        // 36 mm2 a layer standing, against 480 mm2 lying down: the extra layers cost less
        // than the peel would.
        let long_axis = placed(found.rotation, Vec3::Z);
        assert!(
            long_axis.z.abs() > 0.99,
            "the long axis stays vertical, got {long_axis}"
        );
    }

    #[test]
    fn a_cube_stands_on_a_face_whichever_one_it_picks() {
        let mesh = cuboid(Vec3::splat(20.0));
        let found = orient(&mesh, &OrientSettings::default()).expect("a cube can be oriented");

        // Every face of a cube is as good as the others, so what is pinned down is that
        // one of them is flat on the plate: an axis of the model points straight down.
        let down: Vec3 = [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .map(|axis| placed(found.rotation, axis))
            .max_by(|a, b| a.z.abs().total_cmp(&b.z.abs()))
            .expect("three axes");
        assert!(
            down.z.abs() > 0.999,
            "a face is flat on the plate, got {down}"
        );
    }

    #[test]
    fn the_rotation_takes_the_chosen_direction_to_the_plate() {
        let down = Vec3::new(1.0, 2.0, -3.0).normalize();
        let turned = rotation_for(down) * down;
        assert!(
            turned.abs_diff_eq(Vec3::NEG_Z, 1e-5),
            "the chosen direction ends up pointing at the plate, got {turned}"
        );
    }

    #[test]
    fn the_peak_section_of_a_box_is_its_footprint() {
        let mesh = cuboid(Vec3::new(10.0, 6.0, 20.0));
        let peak = peak_section_mm2(&mesh, Vec3::NEG_Z, 8).expect("a box slices");
        assert!((peak - 60.0).abs() < 1e-2, "a 10 by 6 section, got {peak}");
    }

    #[test]
    fn a_mesh_with_no_faces_cannot_be_oriented() {
        let error = orient(&Mesh::default(), &OrientSettings::default())
            .expect_err("an empty mesh has no orientation");
        assert!(matches!(error, PlateError::NothingToOrient));
    }
}
