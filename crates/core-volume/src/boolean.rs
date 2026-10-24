use core_csg::{CsgError, Solid};
use core_geometry::{Bvh, Mesh, Scalar, Vec3, signed_volume, split};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::build::{DEFAULT_BUDGET_BYTES, FieldSettings, build};
use crate::cancel::{Cancel, Step};
use crate::csg;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::hollow::{coarsening, lattice_mm};
use crate::mend::readable;
use crate::sdf::Sdf;
use crate::sign::SignMode;
use local::Whole;

mod local;

/// The precision a combination falls back to the field at, as hollowing states it: a fifth
/// of a millimetre where the model's size allows.
const FIELD_PRECISION: Scalar = 0.5;

/// How models are combined into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Boolean {
    /// Everything any of them holds.
    #[default]
    Union,
    /// The first with every other taken out of it.
    Difference,
    /// Only what all of them hold.
    Intersection,
}

/// Models combined into one closed mesh, and how it was cut.
#[derive(Debug, Clone, PartialEq)]
pub struct Combined {
    pub mesh: Mesh,
    /// The lattice the combination was sampled on, in millimetres, when a model was not
    /// a solid the exact cut can read; `None` when it was cut exactly.
    pub voxel_mm: Option<Scalar>,
}

/// `models` combined by `op`, all in one space, the first being what a difference keeps.
///
/// The cut is exact where every model is a closed solid or mends into one, which moves its
/// surface only at the faces it had to patch. Where one does not — a sheet with nothing
/// inside — the models are sampled into fields and combined there, which rounds sharp edges
/// to the lattice; see `docs/design/csg.md`.
pub fn combine(models: &[Mesh], op: Boolean, cancel: Cancel<'_>) -> Result<Combined, VolumeError> {
    checked(models)?;
    let combined = match exact(models, op, cancel) {
        Ok(mesh) => Combined {
            mesh,
            voxel_mm: None,
        },
        Err(_) => sampled(models, op, cancel)?,
    };
    if combined.mesh.faces.is_empty() {
        return Err(VolumeError::NothingLeft);
    }
    Ok(combined)
}

/// `models` combined by `op` exactly, a torn one mended first, or refused: never sampled into
/// a field. What a caller that must keep every edge of the model as it was asks for.
pub fn combine_exact(models: &[Mesh], op: Boolean) -> Result<Mesh, VolumeError> {
    checked(models)?;
    let mesh = exact(models, op, Cancel::never())?;
    if mesh.faces.is_empty() {
        return Err(VolumeError::NothingLeft);
    }
    Ok(mesh)
}

/// `models` combined as `combine_exact` does, where the others stand on the first from the
/// side `toward` points to: only the part of it on their side goes through the engine, so a
/// few letters on a dense model cost what the letters' neighbourhood does. Falls back to the
/// whole model where cutting it would not pay or the halves would not join.
pub fn combine_exact_toward(
    models: &[Mesh],
    op: Boolean,
    toward: Vec3,
) -> Result<Mesh, VolumeError> {
    checked(models)?;
    if op == Boolean::Intersection {
        return combine_exact(models, op);
    }
    match local::local(models, op, toward) {
        Ok(mesh) if !mesh.faces.is_empty() => Ok(mesh),
        // Every reason it was not done near the others is one the whole model is tried for.
        Ok(_) | Err(Whole::NotLocal | Whole::Seam | Whole::Engine) => combine_exact(models, op),
    }
}

/// Refuses fewer than two models to combine, or one with no faces.
fn checked(models: &[Mesh]) -> Result<(), VolumeError> {
    if models.len() < 2 {
        return Err(VolumeError::TooFewModels(models.len()));
    }
    if models.iter().any(|model| model.faces.is_empty()) {
        return Err(VolumeError::EmptyMesh);
    }
    Ok(())
}

fn exact(models: &[Mesh], op: Boolean, cancel: Cancel<'_>) -> Result<Mesh, CsgError> {
    cancel.begin(Step::Reading, models.len());
    let solids: Vec<Solid> = models
        .par_iter()
        .map(|model| {
            let solid = mended(model);
            cancel.advance(1);
            solid
        })
        .collect::<Result<_, _>>()?;
    cancel.begin(Step::Combining, 0);
    let combined = match (op, solids.as_slice()) {
        (Boolean::Union, _) => Solid::union_all(&solids)?,
        (Boolean::Intersection, _) => largest_last(&solids, models, Solid::intersection)?,
        // The others may overlap each other, so they are one solid before they are cut.
        (Boolean::Difference, [first, rest @ ..]) => first.difference(&Solid::union_all(rest)?)?,
        (Boolean::Difference, []) => return Ok(Mesh::default()),
    };
    cancel.begin(Step::Finishing, 0);
    Ok(combined.mesh())
}

/// `model` as the engine reads it, mended first where it refuses it as it stands, and refused
/// still where mending leaves no volume, as a lone sheet closed over itself.
fn mended(model: &Mesh) -> Result<Solid, CsgError> {
    let solid = readable(model)?;
    if solid.is_empty() {
        return Err(CsgError::NotSolid(
            "nothing is left once it is mended".to_owned(),
        ));
    }
    Ok(solid)
}

/// `solids` combined by `op` with the one of the most faces taken in last, so that a dense
/// model under a few small ones goes through the engine once, not once a level of the tree.
fn largest_last(
    solids: &[Solid],
    models: &[Mesh],
    op: fn(&Solid, &Solid) -> Result<Solid, CsgError>,
) -> Result<Solid, CsgError> {
    let largest = (0..models.len())
        .max_by_key(|index| models[*index].faces.len())
        .unwrap_or(0);
    let others: Vec<Solid> = (0..solids.len())
        .filter(|index| *index != largest)
        .map(|index| solids[index].clone())
        .collect();
    op(&solids[largest], &folded(&others, op)?)
}

/// `mesh` as one solid, its shells unioned first: the engine reads shells that cross — a
/// part pushed into a base — without a complaint and then counts the overlap twice. A mesh
/// with a shell wound inward is a cavity in a part, and is read whole.
pub(crate) fn solid_of(mesh: &Mesh) -> Result<Solid, CsgError> {
    match outward_shells(mesh) {
        Some(shells) => union_of(&shells),
        None => Solid::new(mesh),
    }
}

/// `mesh` with its shells unioned into one surface, so that no face is left standing
/// inside another shell; `None` when it is one shell or holds a cavity, and there is
/// nothing to union.
pub(crate) fn united(mesh: &Mesh) -> Option<Result<Mesh, CsgError>> {
    let shells = outward_shells(mesh)?;
    Some(union_of(&shells).map(|solid| solid.mesh()))
}

/// The shells of `mesh` when there are several and every one is wound outward.
fn outward_shells(mesh: &Mesh) -> Option<Vec<Mesh>> {
    let shells = split(mesh);
    (shells.len() >= 2 && shells.iter().all(|shell| signed_volume(shell) >= 0.0)).then_some(shells)
}

fn union_of(shells: &[Mesh]) -> Result<Solid, CsgError> {
    let solids: Vec<Solid> = shells
        .par_iter()
        .map(Solid::new)
        .collect::<Result<_, _>>()?;
    Solid::union_all(&solids)
}

/// `solids` combined pairwise down a tree rather than one by one into a growing whole.
fn folded(
    solids: &[Solid],
    op: fn(&Solid, &Solid) -> Result<Solid, CsgError>,
) -> Result<Solid, CsgError> {
    match solids {
        [] => Ok(Solid::empty()),
        [only] => Ok(only.clone()),
        _ => {
            let (left, right) = solids.split_at(solids.len() / 2);
            let (left, right) = rayon::join(|| folded(left, op), || folded(right, op));
            op(&left?, &right?)
        }
    }
}

fn sampled(models: &[Mesh], op: Boolean, cancel: Cancel<'_>) -> Result<Combined, VolumeError> {
    let area: Scalar = models.iter().map(Mesh::surface_area).sum();
    // No wall to resolve: the lattice is what precision and the models' size allow.
    let asked_mm = lattice_mm(Scalar::MAX, FIELD_PRECISION, area);
    let (built, voxel_mm) = coarsening(asked_mm, |voxel_mm| fields(models, voxel_mm, cancel))?;
    finished(built, op, voxel_mm, cancel)
}

fn fields(models: &[Mesh], voxel_mm: Scalar, cancel: Cancel<'_>) -> Result<Vec<Sdf>, VolumeError> {
    let settings = FieldSettings {
        voxel_mm,
        sign: SignMode::Auto,
        // Each model is priced on its own, so together they stay inside one budget.
        budget_bytes: DEFAULT_BUDGET_BYTES / models.len(),
        ..FieldSettings::default()
    };
    models
        .iter()
        .map(|model| build(model, &Bvh::build(model), &settings, cancel))
        .collect()
}

fn finished(
    fields: Vec<Sdf>,
    op: Boolean,
    voxel_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<Combined, VolumeError> {
    let mut fields = fields.into_iter();
    let mut whole = fields.next().ok_or(VolumeError::EmptyMesh)?;
    for field in fields {
        whole = match op {
            Boolean::Union => csg::union(&whole, &field)?,
            Boolean::Difference => csg::difference(&whole, &field)?,
            Boolean::Intersection => csg::intersection(&whole, &field)?,
        };
    }
    let mesh = extract(&whole, cancel);
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }
    Ok(Combined {
        mesh,
        voxel_mm: Some(voxel_mm),
    })
}

#[cfg(test)]
mod tests;
