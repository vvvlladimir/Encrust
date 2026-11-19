//! Spreading the objects of a plate over its area, a model no slide fits turned the least that
//! does (ADR 0418, `docs/design/document.md`).

use core_geometry::{BoundingVolume, Mesh, Scalar, Transform, Vec2, placed_aabb};
use core_plate::{ArrangeSettings, Arranged, Footprint, PlateArea, Rect, arrange, turned_onto};
use rayon::prelude::*;
use thiserror::Error;

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::supports::standing_meshes;

/// A model with the supports and raft standing under it, as packing sees them.
#[derive(Debug, Clone)]
pub struct Standing<'a> {
    pub mesh: &'a Mesh,
    pub transform: Transform,
    /// The support and raft meshes, in plate coordinates where they were last built.
    pub supports: Vec<&'a Mesh>,
    /// How the supports moved since: the turn the model was given, which they regrow after.
    pub moved: Transform,
}

/// A model that covers no cell of the plate, so cannot be packed: the index in the slice given.
#[derive(Debug, Error)]
#[error("model {0} has no footprint on the plate")]
pub struct NoFootprint(pub usize);

/// The box across the plate the model stands in with its supports, those moved by `one.moved`.
pub fn standing_rect(one: &Standing<'_>) -> Option<Rect> {
    let supports = one
        .supports
        .iter()
        .filter_map(|mesh| placed_aabb(mesh, one.moved));
    let bounds = placed_aabb(one.mesh, one.transform)
        .into_iter()
        .chain(supports)
        .reduce(|a, b| a.merged(&b))?;
    Some(Rect::under(&bounds))
}

/// How what stood under `one` moves for the least turn about the vertical through its middle
/// that brings it onto `area` where no slide does; `None` if none is needed or none fits.
pub fn turn_to_fit(
    one: &Standing<'_>,
    area: PlateArea,
    settings: &ArrangeSettings,
) -> Option<Transform> {
    let still = Standing {
        moved: Transform::default(),
        ..one.clone()
    };
    turned_onto(standing_rect(&still)?, || outline(&still), area, settings)
}

/// `transform` after the model is turned by `moved`, as [`turn_to_fit`] answers it.
pub fn turned_by(transform: Transform, moved: Transform) -> Transform {
    Transform {
        translation: moved.to_matrix().transform_point3(transform.translation),
        ..transform.turned(moved.rotation)
    }
}

/// Every point of the model and its supports as they stand, seen from above.
fn outline(one: &Standing<'_>) -> Vec<Vec2> {
    let matrix = one.transform.to_matrix();
    let model =
        (one.mesh.vertices.par_iter()).map(|vertex| matrix.transform_point3(*vertex).truncate());
    let supports = (one.supports.par_iter())
        .flat_map(|mesh| mesh.vertices.par_iter().map(|vertex| vertex.truncate()));
    model.chain(supports).collect()
}

/// Packs `standing` over `area`; `placed` indexes the slice, and each offset moves the model
/// with its supports.
fn pack(
    standing: &[Standing<'_>],
    area: PlateArea,
    settings: &ArrangeSettings,
) -> Result<Arranged, NoFootprint> {
    let mut footprints = Vec::with_capacity(standing.len());
    for (index, one) in standing.iter().enumerate() {
        let moved = one.moved;
        let parts: Vec<(&Mesh, Transform)> = std::iter::once((one.mesh, one.transform))
            .chain(one.supports.iter().map(|mesh| (*mesh, moved)))
            .collect();
        let footprint = Footprint::of_parts(&parts, settings.cell_mm);
        footprints.push(footprint.ok_or(NoFootprint(index))?);
    }
    Ok(arrange(&footprints, area, settings))
}

/// A plate spread over its area: each model's transform, how many it turned, and the packing.
#[derive(Debug, Clone)]
pub struct Spread {
    pub transforms: Vec<Transform>,
    pub turned: usize,
    pub arranged: Arranged,
}

/// Turns each of `standing` no slide fits the least that does, then packs them over `area`
/// and moves each by its offset; `standing` is left as turned.
pub fn spread(
    standing: &mut [Standing<'_>],
    area: PlateArea,
    settings: &ArrangeSettings,
) -> Result<Spread, NoFootprint> {
    let mut turned = 0;
    for one in standing.iter_mut() {
        if let Some(moved) = turn_to_fit(one, area, settings) {
            one.transform = turned_by(one.transform, moved);
            one.moved = moved;
            turned += 1;
        }
    }
    let arranged = pack(standing, area, settings)?;
    let mut transforms: Vec<Transform> = standing.iter().map(|one| one.transform).collect();
    for placed in &arranged.placed {
        transforms[placed.index].translation += placed.offset_mm.extend(0.0);
    }
    Ok(Spread {
        transforms,
        turned,
        arranged,
    })
}

/// Spreads every printable object of the active plate over a plate `plate_mm` across.
pub(crate) fn apply_arrange(
    document: &mut Document,
    plate_mm: [Scalar; 2],
    clearance_mm: Scalar,
) -> Result<Applied, ApplyError> {
    let ids = document.printable();
    let built: Vec<_> = ids
        .par_iter()
        .map(|id| standing_meshes(document, *id))
        .collect();
    let mut standing = Vec::with_capacity(ids.len());
    for (id, supports) in ids.iter().zip(&built) {
        let (state, meshes) = document.object(*id).ok_or(ApplyError::NoObject(*id))?;
        standing.push(Standing {
            mesh: meshes.printed(),
            transform: state.transform,
            supports: supports.iter().map(|mesh| &**mesh).collect(),
            moved: Transform::default(),
        });
    }
    let settings = ArrangeSettings {
        clearance_mm,
        ..ArrangeSettings::default()
    };
    let area = PlateArea::whole(Vec2::from_array(plate_mm));
    let Spread {
        transforms,
        arranged,
        ..
    } = spread(&mut standing, area, &settings)
        .map_err(|NoFootprint(index)| ApplyError::NoFootprint(ids[index]))?;
    drop(standing);
    for (id, transform) in ids.iter().zip(transforms) {
        document.set_transform(*id, transform);
    }
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Arranged {
            placed: arranged.placed.len(),
            left_out: arranged.left_out.iter().map(|index| ids[*index]).collect(),
        },
    })
}

#[cfg(test)]
mod tests;
