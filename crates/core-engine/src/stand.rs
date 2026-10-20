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

/// Stands supports under `mesh`, lifted first by the profile's lift and raft with `cuts`
/// riding along, and answers with them: `docs/design/supports.md`.
pub fn stand_under(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    stand(mesh, cuts, profile, layer_height_mm, profile.lift_mm())
}

/// The same for a model left standing on the plate: supports go only under what hangs
/// over it, and the plate holds the rest. A model that needs none gets none, and one that
/// needs some stands on the raft's top, as the window's raft stands it.
pub fn stand_seated(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    let (mut lifted_mm, mut drilled, mut contacts) =
        touched(mesh, cuts, profile, layer_height_mm, 0.0)?;
    if profile.raft.enabled && !contacts.is_empty() {
        let up = Vec3::Z * profile.raft.thickness_mm;
        let moved = Transform::from_translation(up);
        *mesh = transform_mesh(mesh, moved);
        if let Some(cuts) = cuts.as_mut() {
            *cuts = transform_mesh(cuts, moved);
        }
        drilled = transform_mesh(&drilled, moved);
        for contact in &mut contacts {
            *contact += up;
        }
        lifted_mm += up.z;
    }
    held(lifted_mm, &drilled, contacts, profile)
}

fn stand(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
    lift_mm: Scalar,
) -> Result<Stood, EngineError> {
    let (lifted_mm, drilled, contacts) = touched(mesh, cuts, profile, layer_height_mm, lift_mm)?;
    held(lifted_mm, &drilled, contacts, profile)
}

/// Columns stood under `contacts` on `drilled`, and what they came to.
fn held(
    lifted_mm: Scalar,
    drilled: &Mesh,
    contacts: Vec<Vec3>,
    profile: &SupportProfile,
) -> Result<Stood, EngineError> {
    let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();

    let bvh = Bvh::build(drilled);
    let placed = Placed::new(drilled, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).ok_or(EngineError::EmptyPlate)?;

    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    let facing = facing(drilled, &bvh, &contacts);
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
            bvh.closest(*contact)
                .and_then(|hit| drilled.triangle(hit.face))
                .map_or(Vec3::ZERO, |triangle| {
                    triangle.normal_unnormalized().normalize_or_zero()
                })
        })
        .collect()
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

    /// Axis-aligned box from `low` to `high`, wound outward.
    fn cuboid(mesh: &mut Mesh, low: Vec3, high: Vec3) {
        let corner = |at: usize| {
            Vec3::new(
                if at & 1 == 0 { low.x } else { high.x },
                if at & 2 == 0 { low.y } else { high.y },
                if at & 4 == 0 { low.z } else { high.z },
            )
        };
        let box_mesh = Mesh::new(
            (0..8).map(corner).collect(),
            vec![
                [0, 2, 1],
                [1, 2, 3],
                [4, 5, 6],
                [5, 7, 6],
                [0, 1, 4],
                [1, 5, 4],
                [2, 6, 3],
                [3, 6, 7],
                [0, 4, 2],
                [2, 4, 6],
                [1, 3, 5],
                [3, 7, 5],
            ],
        );
        append(mesh, &box_mesh);
    }

    /// A 10 mm slab on a 2 mm stem standing on the plate: its underside hangs.
    fn mushroom() -> Mesh {
        let mut mesh = Mesh::default();
        cuboid(
            &mut mesh,
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(6.0, 6.0, 11.0),
        );
        cuboid(
            &mut mesh,
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(10.0, 10.0, 12.0),
        );
        mesh
    }

    #[test]
    fn a_seated_model_with_supports_stands_on_its_rafts_top() {
        let mut profile = SupportProfile::medium();
        profile.raft.enabled = true;
        let mut mesh = mushroom();
        let stood = stand_seated(&mut mesh, &mut None, &profile, 0.05)
            .expect("a mushroom on the plate can be supported");
        let low_mm = mesh.aabb().expect("the mushroom has bounds").mins.z;

        assert!(!stood.contacts.is_empty(), "the slab's underside hangs");
        assert!(
            (low_mm - profile.raft.thickness_mm).abs() < 1e-4,
            "the raft is not printed into the stem, got {low_mm}"
        );
        assert!((stood.lifted_mm - profile.raft.thickness_mm).abs() < 1e-4);
    }

    #[test]
    fn a_seated_model_needing_no_supports_stays_on_the_plate_over_a_raft() {
        let mut profile = SupportProfile::medium();
        profile.raft.enabled = true;
        let mut mesh = Mesh::default();
        cuboid(&mut mesh, Vec3::ZERO, Vec3::splat(10.0));
        let stood = stand_seated(&mut mesh, &mut None, &profile, 0.05)
            .expect("a cube on the plate can be supported");

        assert!(stood.contacts.is_empty(), "nothing on a cube hangs");
        assert!(stood.lifted_mm.abs() < 1e-6, "no raft is built, so no lift");
    }
}
