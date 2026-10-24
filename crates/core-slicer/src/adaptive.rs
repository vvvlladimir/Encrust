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

/// Thickness is the cusp target over the steepest lean; flat faces ask for a boundary of their own.
/// See `docs/design/slicing.md`.
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
    // Counted from the last boundary off the step grid: adding a thousand f32 thicknesses
    // drifts enough to leave the model's top short and buy a layer with nothing in it.
    let mut anchor = z_min;
    let mut steps_above = 0usize;
    let mut z = z_min;
    let mut next_flat = 0;
    while z < z_max {
        while flats
            .get(next_flat)
            .is_some_and(|&flat| flat <= z + APART_MM)
        {
            next_flat += 1;
        }
        let steps = walk(&lean, z, z_min, step, settings);
        if let Some(reach) = reach_flat(
            steps as Scalar * step,
            flats.get(next_flat).map(|&flat| flat - z),
            settings,
        ) {
            anchor = z + reach;
            steps_above = 0;
            z = anchor;
        } else {
            steps_above += steps;
            z = anchor + steps_above as Scalar * step;
        }
        bounds.push(z);
    }
    Ok(LayerPlan::from_bounds(bounds, z_max))
}

/// How far a layer below a flat face must reach to land on it, or `None` if the thickness stands.
fn reach_flat(
    thickness: Scalar,
    to_flat: Option<Scalar>,
    settings: &AdaptiveSettings,
) -> Option<Scalar> {
    let reach = to_flat?;
    (reach >= settings.min_height_mm && reach < thickness).then_some(reach)
}

/// How many whole `step`s thick the layer standing on `z` may be: the thinnest the surface
/// over the whole band allows, rounded down.
fn walk(
    lean: &[Scalar],
    z: Scalar,
    z_min: Scalar,
    step: Scalar,
    settings: &AdaptiveSettings,
) -> usize {
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
    taken.max(1)
}

/// Per `step`-tall slot, the sine of the steepest lean of any face crossing it; flat faces
/// excluded.
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
mod tests;
