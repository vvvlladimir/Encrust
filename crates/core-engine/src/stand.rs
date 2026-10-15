use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, lift_over_plate, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;

use crate::error::EngineError;

/// What automatic placement put under one model.
#[derive(Debug, Clone, PartialEq)]
pub struct Stood {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    /// The way the surface under each contact faces, unit, or zero where none was found.
    pub facing: Vec<Vec3>,
    /// Contacts a column was stood under.
    pub standing: usize,
    pub trees: usize,
    /// The supports themselves, in plate coordinates.
    pub supports: Mesh,
}

/// Stands supports under `mesh`, in plate coordinates, and answers with them.
///
/// The model is lifted off the plate first, by the profile's own lift and its raft, and `cuts`
/// rides the lift with it. Contacts are found on the drilled model, so nothing stands
/// under a surface a hole has taken away. See `docs/design/supports.md`.
pub fn stand_under(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    stand(mesh, cuts, profile, layer_height_mm, profile.lift_mm())
}

/// The same for a model left standing on the plate: supports go only under what hangs
/// over it, and the plate holds the rest. A model that needs none gets none.
pub fn stand_seated(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    stand(mesh, cuts, profile, layer_height_mm, 0.0)
}

fn stand(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
    lift_mm: Scalar,
) -> Result<Stood, EngineError> {
    let (lifted_mm, drilled, contacts) = touched(mesh, cuts, profile, layer_height_mm, lift_mm)?;
    let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();

    let bvh = Bvh::build(&drilled);
    let placed = Placed::new(&drilled, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).ok_or(EngineError::EmptyPlate)?;

    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    let facing = facing(&drilled, &bvh, &contacts);
    Ok(Stood {
        lifted_mm,
        contacts,
        facing,
        standing: trees
            .iter()
            .map(core_supports::SupportTree::tip_count)
            .sum(),
        trees: trees.len(),
        supports: mesh_trees(&trees, &placed, profiles),
    })
}

/// `mesh` and what is cut out of it lifted `lift_mm` clear of the plate, then how far they
/// moved, the drilled model, and where it needs holding up.
fn touched(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
    lift_mm: Scalar,
) -> Result<(Scalar, Mesh, Vec<Vec3>), EngineError> {
    let lifted_mm = lift(mesh, cuts, lift_mm)?;
    let mut drilled = mesh.clone();
    if let Some(cuts) = cuts.as_ref() {
        append(&mut drilled, cuts);
    }
    let contacts = contacts_under(&drilled, profile, layer_height_mm)?;
    Ok((lifted_mm, drilled, contacts))
}

/// The way the surface of `drilled` under each contact faces, unit, or zero where the
/// contact finds none.
fn facing(drilled: &Mesh, bvh: &Bvh, contacts: &[Vec3]) -> Vec<Vec3> {
    contacts
        .iter()
        .map(|contact| {
            bvh.closest(drilled, *contact)
                .and_then(|hit| drilled.triangle(hit.face))
                .map_or(Vec3::ZERO, |triangle| {
                    triangle.normal_unnormalized().normalize_or_zero()
                })
        })
        .collect()
}

/// A face this near the lowest point of a model set on the plate is held by the plate.
const SEATED_MM: Scalar = 0.5;

/// How many places `placed`, set bare on the plate, leaves hanging: what placement would
/// stand under it lifted clear, less what would only have held its seat. Zero is a model
/// that prints with no supports.
pub fn left_hanging(
    placed: &Mesh,
    cuts: Option<&Mesh>,
    support: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<usize, EngineError> {
    let lift_mm = support.lift_mm();
    let lifted = Transform::from_translation(Vec3::Z * lift_mm);
    let mut drilled = transform_mesh(placed, lifted);
    if let Some(cuts) = cuts {
        append(&mut drilled, &transform_mesh(cuts, lifted));
    }
    let contacts = contacts_under(&drilled, support, layer_height_mm)?;
    Ok(contacts
        .iter()
        .filter(|contact| contact.z > lift_mm + SEATED_MM)
        .count())
}

/// Where `drilled`, standing where it is, needs holding up: what automatic placement
/// would touch, before anything is stood under it.
fn contacts_under(
    drilled: &Mesh,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<Vec<Vec3>, EngineError> {
    let stack = PlaneSliceEngine
        .slice(
            drilled,
            &SliceSettings {
                layer_height: layer_height_mm,
                ..SliceSettings::default()
            },
        )
        .map_err(EngineError::Supports)?;
    let lows = core_supports::low_points(drilled, Transform::default());
    Ok(core_supports::generate_supports(
        &stack,
        layer_height_mm,
        profile,
        &[],
        &lows,
        None,
        &mut |_| true,
    ))
}

/// Stands `mesh` and what is cut out of it `lift_mm` clear of the plate, and answers how
/// far they moved. A model already standing at least that high is left where it is: the
/// lift makes room under a part, it does not place it.
fn lift(mesh: &mut Mesh, cuts: &mut Option<Mesh>, lift_mm: Scalar) -> Result<Scalar, EngineError> {
    let bounds = mesh.aabb().ok_or(EngineError::EmptyPlate)?;
    if bounds.mins.z >= lift_mm {
        return Ok(0.0);
    }
    let offset = lift_over_plate(&bounds, lift_mm);
    let moved = Transform::from_translation(offset);
    *mesh = transform_mesh(mesh, moved);
    if let Some(cuts) = cuts.as_mut() {
        *cuts = transform_mesh(cuts, moved);
    }
    Ok(offset.z)
}

/// Adds `other`'s geometry to `mesh`, keeping each face pointing at its own vertices.
fn append(mesh: &mut Mesh, other: &Mesh) {
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|face| [face[0] + offset, face[1] + offset, face[2] + offset]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_with_no_vertices_has_nothing_to_stand_under() {
        let error = stand_under(
            &mut Mesh::default(),
            &mut None,
            &SupportProfile::medium(),
            0.05,
        )
        .expect_err("an empty mesh cannot be supported");
        assert!(matches!(error, EngineError::EmptyPlate));
    }

    #[test]
    fn a_layer_of_no_height_cannot_be_cut_for_contacts() {
        let mut slab = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 10.0, 10.0),
                Vec3::new(0.0, 0.0, 12.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let error = stand_under(&mut slab, &mut None, &SupportProfile::medium(), 0.0)
            .expect_err("no layer height, no stack");
        assert!(matches!(error, EngineError::Supports(_)));
    }
}
