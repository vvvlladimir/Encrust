use core_geometry::{Mesh, Scalar};
use rayon::prelude::*;

use serde::{Deserialize, Serialize};

use crate::engine::on_the_plate;
use crate::{LayerPlan, SliceError};

/// What an adaptive stack is allowed to do: how much stair-stepping to accept, and the
/// thicknesses it may reach for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveSettings {
    /// Stair-step the surface may show, millimetres. Smaller means thinner layers where
    /// the surface is shallow; see `docs/design/slicing.md`.
    pub cusp_mm: Scalar,
    pub min_height_mm: Scalar,
    pub max_height_mm: Scalar,
}

impl Default for AdaptiveSettings {
    /// A tenth of the finest layer most machines print, against the 0.02 to 0.10 mm range
    /// an MSLA panel is worth slicing in.
    fn default() -> Self {
        Self {
            cusp_mm: 0.01,
            min_height_mm: 0.02,
            max_height_mm: 0.10,
        }
    }
}

impl AdaptiveSettings {
    /// The thickest layer these settings can actually produce: every thickness is a whole
    /// number of `min_height_mm`, so a ceiling that is not one is never reached.
    pub fn reachable_max_mm(&self) -> Scalar {
        if self.min_height_mm <= 0.0 {
            return self.max_height_mm;
        }
        let steps = (self.max_height_mm / self.min_height_mm).floor().max(1.0);
        steps * self.min_height_mm
    }

    fn validate(&self) -> Result<(), SliceError> {
        if self.min_height_mm <= 0.0 {
            return Err(SliceError::NonPositiveLayerHeight(self.min_height_mm));
        }
        if self.max_height_mm < self.min_height_mm {
            return Err(SliceError::UpsideDownRange {
                min_mm: self.min_height_mm,
                max_mm: self.max_height_mm,
            });
        }
        if self.cusp_mm <= 0.0 {
            return Err(SliceError::NonPositiveCusp(self.cusp_mm));
        }
        Ok(())
    }
}

/// How near vertical a normal has to point for a face to stair-step at all, and how near
/// horizontal before it stops stair-stepping and asks for a boundary on itself instead.
const HORIZONTAL: Scalar = 0.999;

/// Millimetres two boundaries have to differ by to be worth keeping apart.
const APART_MM: Scalar = 1e-4;

/// A plan whose layers are as thick as the surface running through them allows.
///
/// The thickness a height may take is the cusp target over how far the steepest surface
/// there leans away from vertical: a wall takes the thickest layer going, a shallow slope
/// the thinnest. A flat face does not stair-step at all — it asks for a boundary of its
/// own, and gets one. See `docs/design/slicing.md`.
pub fn plan(mesh: &Mesh, settings: &AdaptiveSettings) -> Result<LayerPlan, SliceError> {
    plan_under(mesh, settings, Scalar::INFINITY)
}

/// The same, with nothing planned above `ceiling_mm`; see [`crate::layer_heights_under`].
pub fn plan_under(
    mesh: &Mesh,
    settings: &AdaptiveSettings,
    ceiling_mm: Scalar,
) -> Result<LayerPlan, SliceError> {
    settings.validate()?;
    let (z_min, z_max) = on_the_plate(mesh, ceiling_mm)?;
    if z_max <= z_min {
        return Ok(LayerPlan::from_bounds(Vec::new(), z_max));
    }

    let step = settings.min_height_mm;
    let lean = steepest_lean(mesh, z_min, z_max, step);
    let flats = flat_heights(mesh, z_min, z_max);

    let mut bounds = vec![z_min];
    let mut z = z_min;
    let mut next_flat = 0;
    while z < z_max {
        while flats
            .get(next_flat)
            .is_some_and(|&flat| flat <= z + APART_MM)
        {
            next_flat += 1;
        }
        let thickness = reach_flat(
            walk(&lean, z, z_min, step, settings),
            flats.get(next_flat).map(|&flat| flat - z),
            settings,
        );
        z += thickness;
        bounds.push(z);
    }
    Ok(LayerPlan::from_bounds(bounds, z_max))
}

/// A layer that could land exactly on a flat face does, because a boundary on a flat face
/// reproduces it with no error at all, which no thickness can do.
///
/// Only ever a shorter layer: stretching one to reach a flat face would skip the thin
/// layers the surface below it asked for.
fn reach_flat(thickness: Scalar, to_flat: Option<Scalar>, settings: &AdaptiveSettings) -> Scalar {
    let Some(reach) = to_flat else {
        return thickness;
    };
    if reach >= settings.min_height_mm && reach < thickness {
        return reach;
    }
    thickness
}

/// How thick the layer standing on `z` may be: the thinnest the surface over the whole
/// band allows, rounded down to a whole number of `step`.
fn walk(
    lean: &[Scalar],
    z: Scalar,
    z_min: Scalar,
    step: Scalar,
    settings: &AdaptiveSettings,
) -> Scalar {
    let steps = (settings.max_height_mm / step).floor().max(1.0) as usize;
    let first = ((z - z_min) / step).floor().max(0.0) as usize;

    let mut allowed = settings.max_height_mm;
    let mut taken = 0;
    for offset in 0..steps {
        let slot = lean.get(first + offset).copied().unwrap_or(0.0);
        if slot > 0.0 {
            allowed = allowed.min(settings.cusp_mm / slot);
        }
        if (offset + 1) as Scalar * step > allowed + APART_MM {
            break;
        }
        taken = offset + 1;
    }
    taken.max(1) as Scalar * step
}

/// For each `step`-tall slot of the model, the sine of the steepest lean any face
/// crossing it shows: zero where nothing crosses, one just short of a flat ceiling.
///
/// A face is scattered into every slot it spans rather than queried per slot, so the cost
/// follows the faces and not the height of the model. A flat face is left out: it is a
/// boundary to land on, not a surface to slice thinner for.
fn steepest_lean(mesh: &Mesh, z_min: Scalar, z_max: Scalar, step: Scalar) -> Vec<Scalar> {
    let slots = (((z_max - z_min) / step).ceil() as usize).max(1);
    let slot_of = |z: Scalar| (((z - z_min) / step).floor().max(0.0) as usize).min(slots - 1);

    mesh.faces
        .par_iter()
        .fold(
            || vec![0.0 as Scalar; slots],
            |mut lean, face| {
                let Some((points, steepness)) = face_lean(mesh, face) else {
                    return lean;
                };
                if steepness >= HORIZONTAL {
                    return lean;
                }
                let low = slot_of(points.iter().fold(Scalar::INFINITY, |z, p| z.min(p.z)));
                let high = slot_of(points.iter().fold(Scalar::NEG_INFINITY, |z, p| z.max(p.z)));
                for slot in &mut lean[low..=high] {
                    *slot = slot.max(steepness);
                }
                lean
            },
        )
        .reduce(
            || vec![0.0 as Scalar; slots],
            |mut into, from| {
                for (slot, value) in into.iter_mut().zip(from) {
                    *slot = slot.max(value);
                }
                into
            },
        )
}

/// The heights of the model's flat faces, in order and without repeats: every place a
/// layer boundary reproduces the surface exactly.
fn flat_heights(mesh: &Mesh, z_min: Scalar, z_max: Scalar) -> Vec<Scalar> {
    let mut heights: Vec<Scalar> = mesh
        .faces
        .iter()
        .filter_map(|face| {
            let (points, steepness) = face_lean(mesh, face)?;
            let z = (points[0].z + points[1].z + points[2].z) / 3.0;
            (steepness >= HORIZONTAL && z > z_min && z < z_max).then_some(z)
        })
        .collect();
    heights.sort_by(f32::total_cmp);
    heights.dedup_by(|a, b| (*a - *b).abs() < APART_MM);
    heights
}

/// A face's corners and how far it leans away from vertical, or `None` where it has no
/// area to have a normal.
fn face_lean(mesh: &Mesh, face: &[u32; 3]) -> Option<([core_geometry::Vec3; 3], Scalar)> {
    let points = face.map(|index| mesh.vertices[index as usize]);
    let normal = (points[1] - points[0])
        .cross(points[2] - points[0])
        .try_normalize()?;
    // The cusp a stair step leaves is the layer height times the cosine of the angle
    // between the surface normal and Z; see docs/design/slicing.md.
    Some((points, normal.z.abs()))
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;

    /// An axis-aligned box from the origin to `size`, wound outward.
    fn box_mesh(size: Vec3) -> Mesh {
        let corners = [
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
        Mesh::new(corners.to_vec(), faces)
    }

    /// A wedge whose top face leans at 45 degrees: a right triangular prism lying along Y.
    fn ramp(run: Scalar, rise: Scalar) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(run, 0.0, 0.0),
            Vec3::new(run, 0.0, rise),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(run, 1.0, 0.0),
            Vec3::new(run, 1.0, rise),
        ];
        let faces = vec![
            [0, 1, 2],
            [3, 5, 4],
            [0, 2, 5],
            [0, 5, 3],
            [1, 4, 5],
            [1, 5, 2],
            [0, 3, 4],
            [0, 4, 1],
        ];
        Mesh::new(vertices, faces)
    }

    fn settings() -> AdaptiveSettings {
        AdaptiveSettings {
            cusp_mm: 0.05,
            min_height_mm: 0.05,
            max_height_mm: 0.20,
        }
    }

    #[test]
    fn a_box_is_sliced_at_the_thickest_layer_allowed() {
        let plan = plan(&box_mesh(Vec3::new(2.0, 2.0, 2.0)), &settings()).expect("a box plans");

        assert_eq!(
            plan.layer_count(),
            10,
            "2 mm of vertical wall at the 0.2 mm ceiling is ten layers"
        );
        assert!(plan.is_uniform());
    }

    #[test]
    fn a_forty_five_degree_ramp_is_sliced_thinner_than_the_ceiling() {
        // cos 45 is 0.707, so a 0.05 mm cusp allows 0.0707 mm, which is one 0.05 mm step.
        let plan = plan(&ramp(1.0, 1.0), &settings()).expect("a ramp plans");

        assert_eq!(
            plan.layer_count(),
            20,
            "1 mm of ramp at 0.05 mm is 20 layers"
        );
        for index in 0..plan.layer_count() {
            let thickness = plan.thickness_of(index).expect("a layer");
            assert!(
                (thickness - 0.05).abs() < 1e-5,
                "layer {index} is {thickness} mm, not the 0.05 mm a 45 degree face allows"
            );
        }
    }

    /// Two boxes stacked, the lower one ending at `shelf`: the step between them is a
    /// flat face at a height the layer grid would otherwise miss.
    fn stepped(shelf: Scalar) -> Mesh {
        let lower = box_mesh(Vec3::new(4.0, 4.0, shelf));
        let upper = core_geometry::transform_mesh(
            &box_mesh(Vec3::new(2.0, 2.0, 2.0 - shelf)),
            core_geometry::Transform::from_translation(Vec3::new(0.0, 0.0, shelf)),
        );
        let offset = lower.vertices.len() as u32;
        let mut vertices = lower.vertices;
        vertices.extend(upper.vertices);
        let mut faces = lower.faces;
        faces.extend(upper.faces.iter().map(|face| face.map(|i| i + offset)));
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_shelf_gets_a_boundary_of_its_own_rather_than_the_nearest_grid_line() {
        let shelf = 0.93;
        let plan = plan(&stepped(shelf), &settings()).expect("a stepped model plans");

        let landed = (0..plan.layer_count())
            .filter_map(|index| plan.top_of(index))
            .any(|top| (top - shelf).abs() < 1e-5);
        assert!(
            landed,
            "no layer tops out at the {shelf} mm shelf, so the step prints at the wrong height"
        );
    }

    #[test]
    fn a_shallow_ramp_is_sliced_thinner_than_a_steep_one() {
        let steep = plan(&ramp(1.0, 4.0), &settings()).expect("a steep ramp plans");
        let shallow = plan(&ramp(4.0, 4.0), &settings()).expect("a shallow ramp plans");

        assert!(
            shallow.layer_count() > steep.layer_count(),
            "the same 4 mm of rise takes {} layers shallow against {} steep",
            shallow.layer_count(),
            steep.layer_count()
        );
    }

    #[test]
    fn every_layer_stays_inside_the_range_and_the_stack_covers_the_model() {
        let settings = AdaptiveSettings {
            cusp_mm: 0.02,
            min_height_mm: 0.03,
            max_height_mm: 0.12,
        };
        let plan = plan(&ramp(3.0, 5.0), &settings).expect("a ramp plans");

        let mut total = 0.0;
        for index in 0..plan.layer_count() {
            let thickness = plan.thickness_of(index).expect("a layer");
            assert!(
                thickness <= settings.max_height_mm + 1e-5,
                "layer {index} is {thickness} mm, over the ceiling"
            );
            total += thickness;
        }
        assert!(
            (5.0..5.0 + settings.max_height_mm).contains(&total),
            "the stack covers {total} mm of a 5 mm model: it may overshoot by under a layer"
        );
    }

    #[test]
    fn a_ceiling_that_is_not_a_whole_number_of_the_floor_is_never_reached() {
        let settings = AdaptiveSettings {
            cusp_mm: 0.05,
            min_height_mm: 0.023,
            max_height_mm: 0.05,
        };
        assert!((settings.reachable_max_mm() - 0.046).abs() < 1e-6);

        let divisible = AdaptiveSettings {
            min_height_mm: 0.025,
            ..settings
        };
        assert!((divisible.reachable_max_mm() - 0.05).abs() < 1e-6);
    }

    #[test]
    fn a_range_that_is_upside_down_is_rejected() {
        let broken = AdaptiveSettings {
            min_height_mm: 0.1,
            max_height_mm: 0.05,
            ..settings()
        };
        assert!(matches!(
            plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
            SliceError::UpsideDownRange { .. }
        ));
    }

    #[test]
    fn a_cusp_of_zero_is_rejected() {
        let broken = AdaptiveSettings {
            cusp_mm: 0.0,
            ..settings()
        };
        assert!(matches!(
            plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
            SliceError::NonPositiveCusp(_)
        ));
    }

    #[test]
    fn a_zero_thickness_floor_is_rejected_and_an_empty_mesh_too() {
        let broken = AdaptiveSettings {
            min_height_mm: 0.0,
            ..settings()
        };
        assert!(matches!(
            plan(&box_mesh(Vec3::ONE), &broken).unwrap_err(),
            SliceError::NonPositiveLayerHeight(_)
        ));
        assert!(matches!(
            plan(&Mesh::default(), &settings()).unwrap_err(),
            SliceError::EmptyMesh
        ));
    }
}
