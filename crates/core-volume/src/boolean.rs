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
/// The cut is exact where every model is a closed solid. Where one is not — an open scan,
/// a shell crossing itself — the models are sampled into fields and combined there, which
/// rounds sharp edges to the lattice; see `docs/design/csg.md`.
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

/// `models` combined by `op` exactly, or refused: never sampled into a field. What a caller
/// that must keep every edge of the model as it was asks for.
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
            let solid = solid_of(model);
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
fn solid_of(mesh: &Mesh) -> Result<Solid, CsgError> {
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
mod tests {
    use core_geometry::{Vec3, cuboid, diagnose};

    use super::*;

    /// A cube `side` across from `low`.
    fn cube(low: Scalar, side: Scalar) -> Mesh {
        cuboid(Vec3::splat(low), Vec3::splat(low + side))
    }

    /// Two 20 mm cubes overlapping in a 10 mm cube at their shared corner.
    fn pair() -> Vec<Mesh> {
        vec![cube(0.0, 20.0), cube(10.0, 20.0)]
    }

    fn volume(combined: &Combined) -> Scalar {
        assert_eq!(
            diagnose(&combined.mesh).boundary_edges,
            0,
            "a combination is closed"
        );
        signed_volume(&combined.mesh)
    }

    #[test]
    fn an_exact_combination_ends_on_making_its_mesh() {
        let progress = crate::Progress::default();
        let combined = combine(
            &pair(),
            Boolean::Union,
            Cancel::never().reporting(&progress),
        )
        .expect("two cubes combine");

        assert_eq!(combined.voxel_mm, None, "two solids combine exactly");
        assert_eq!(progress.read(), Some((Step::Finishing, 0, 0)));
    }

    #[test]
    fn two_solids_combine_exactly_by_their_shared_corner() {
        for (op, expected) in [
            (Boolean::Union, 15_000.0),
            (Boolean::Difference, 7_000.0),
            (Boolean::Intersection, 1_000.0),
        ] {
            let combined = combine(&pair(), op, Cancel::never()).expect("two cubes combine");
            assert_eq!(combined.voxel_mm, None, "{op:?} of two solids is exact");
            let got = volume(&combined);
            assert!(
                (got - expected).abs() < 1e-2,
                "{op:?}: 8000 mm³ each, 1000 shared, so {expected}; got {got}"
            );
        }
    }

    #[test]
    fn combining_toward_a_side_answers_what_combining_whole_does() {
        // A 20 mm cube with a 5 mm one standing half out of its top, seen from above and
        // from below, where the cut would take the whole cube and is not made.
        let models = vec![
            cube(0.0, 20.0),
            cuboid(Vec3::new(5.0, 5.0, 17.5), Vec3::new(10.0, 10.0, 22.5)),
        ];
        for toward in [Vec3::Z, -Vec3::Z] {
            for (op, expected) in [
                (Boolean::Union, 8_000.0 + 62.5),
                (Boolean::Difference, 8_000.0 - 62.5),
                (Boolean::Intersection, 62.5),
            ] {
                let mesh = combine_exact_toward(&models, op, toward).expect("both are solids");
                let got = volume(&Combined {
                    mesh,
                    voxel_mm: None,
                });
                assert!(
                    (got - expected).abs() < 1e-2,
                    "{op:?} toward {toward}: 8000 mm³ and half of 125 outside it; got {got}"
                );
            }
        }
    }

    #[test]
    fn combining_toward_a_side_refuses_what_combining_whole_does() {
        assert_eq!(
            combine_exact_toward(&[cube(0.0, 1.0)], Boolean::Union, Vec3::Z),
            Err(VolumeError::TooFewModels(1))
        );
        assert_eq!(
            combine_exact_toward(&[cube(0.0, 1.0), Mesh::default()], Boolean::Union, Vec3::Z),
            Err(VolumeError::EmptyMesh)
        );
        let mut open = cube(0.0, 20.0);
        open.faces.truncate(10);
        assert!(matches!(
            combine_exact_toward(&[open, cube(5.0, 20.0)], Boolean::Union, Vec3::Z),
            Err(VolumeError::Csg(CsgError::NotSolid(_)))
        ));
    }

    #[test]
    fn a_difference_takes_every_other_model_out_of_the_first() {
        // Two cubes overlapping each other inside the first are taken out once.
        let models = vec![cube(0.0, 20.0), cube(5.0, 10.0), cube(10.0, 10.0)];
        let got = volume(&combine(&models, Boolean::Difference, Cancel::never()).expect("cut"));
        let expected = 8_000.0 - (1_000.0 + 1_000.0 - 125.0);
        assert!(
            (got - expected).abs() < 1e-2,
            "8000 less two 1000 mm³ cubes sharing 125; got {got}"
        );
    }

    #[test]
    fn a_model_that_is_not_a_solid_is_combined_through_the_field() {
        let mut open = cube(10.0, 20.0);
        open.faces.truncate(10);
        let models = vec![cube(0.0, 20.0), open];
        let combined =
            combine(&models, Boolean::Union, Cancel::never()).expect("the field answers");
        assert!(
            combined.voxel_mm.is_some(),
            "an open cube cannot be cut exactly"
        );
        let got = volume(&combined);
        assert!(
            (got - 15_000.0).abs() < 0.03 * 15_000.0,
            "within the lattice's rounding of the exact 15000; got {got}"
        );
    }

    #[test]
    fn a_model_of_crossing_shells_is_unioned_before_it_is_cut() {
        // Two cubes in one mesh, crossing in a 10 mm cube: 15000 mm³ as one solid.
        let mut crossing = cube(0.0, 20.0);
        crossing.append(&cube(10.0, 20.0));
        let models = vec![crossing, cube(40.0, 10.0)];
        let combined = combine(&models, Boolean::Union, Cancel::never()).expect("union");
        assert_eq!(
            combined.voxel_mm, None,
            "crossing shells are still cut exactly"
        );
        let got = volume(&combined);
        assert!(
            (got - 16_000.0).abs() < 1e-2,
            "15000 for the pair and 1000 apart; got {got}"
        );
    }

    #[test]
    fn an_exact_combination_refuses_what_it_cannot_read() {
        let mut open = cube(10.0, 20.0);
        open.faces.truncate(10);
        assert!(matches!(
            combine_exact(&[cube(0.0, 20.0), open], Boolean::Union),
            Err(VolumeError::Csg(CsgError::NotSolid(_)))
        ));
        assert_eq!(
            combine_exact(&[cube(0.0, 10.0), cube(20.0, 10.0)], Boolean::Intersection),
            Err(VolumeError::NothingLeft)
        );
    }

    #[test]
    fn models_that_share_nothing_intersect_in_nothing() {
        let apart = vec![cube(0.0, 10.0), cube(20.0, 10.0)];
        assert_eq!(
            combine(&apart, Boolean::Intersection, Cancel::never()),
            Err(VolumeError::NothingLeft)
        );
    }

    #[test]
    fn one_model_or_an_empty_one_is_refused() {
        assert_eq!(
            combine(&[cube(0.0, 1.0)], Boolean::Union, Cancel::never()),
            Err(VolumeError::TooFewModels(1))
        );
        assert_eq!(
            combine(
                &[cube(0.0, 1.0), Mesh::default()],
                Boolean::Union,
                Cancel::never()
            ),
            Err(VolumeError::EmptyMesh)
        );
    }

    #[test]
    fn a_run_given_up_on_stops_in_the_field() {
        let mut open = cube(10.0, 20.0);
        open.faces.truncate(10);
        let stop = || true;
        assert_eq!(
            combine(
                &[cube(0.0, 20.0), open],
                Boolean::Union,
                Cancel::when(&stop)
            ),
            Err(VolumeError::Cancelled)
        );
    }
}
