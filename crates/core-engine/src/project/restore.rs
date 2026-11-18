//! What a project's models are put back together from: the file holds every decision, so
//! opening one only meshes and cuts what the decisions already fix (ADR 0191).

use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Transform};
use core_supports::ModelSupports;
use core_volume::{DrainHole, ModelHollow, Shell, drilled_out};
use printer_profiles::SupportProfile;

use super::{ModelMeshes, ObjectHollowState, ObjectSupportState};

/// The cavity an object carries, as the file left it: its shell is read, not hollowed
/// again, and the holes are cut against the wall that shell stands at.
pub fn hollow_of(state: &ObjectHollowState, meshes: &ModelMeshes) -> ModelHollow {
    let mut hollow = ModelHollow::restored(
        state.blockers.clone(),
        state.drains.clone(),
        state.channels.clone(),
    );
    let (Some(built), Some(mesh)) = (&state.built, &meshes.shell) else {
        return hollow;
    };
    let settings = hollow.asking(&built.wall.settings());
    hollow.take(Shell {
        mesh: Arc::clone(mesh),
        cavity: built.cavity_faces.clone(),
        cavity_mm3: built.cavity_mm3,
        voxel_mm: built.voxel_mm,
        coarsened: built.coarsened,
        shape: built.shape(),
        settings,
    });
    hollow
}

/// The supports an object carries, standing where the file says they stand: the trees are
/// read and meshed against `surface`, whose hierarchy is `bvh`, and no run decides where a
/// column goes again. The patches are `model`'s. A tip one of `drains` drilled out goes, and
/// the trees grow again without it (ADR 0443).
pub fn supports_of(
    state: &ObjectSupportState,
    drains: &[DrainHole],
    model: &Mesh,
    surface: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    table: &[SupportProfile],
) -> ModelSupports {
    let mut supports = ModelSupports::restore(
        state.points.clone(),
        state.painted.clone(),
        state.blocked.clone(),
        state.frozen.clone(),
    );
    let taken = supports.clear_contacts(|contact| drilled_out(drains, contact));
    let grown = if taken > 0 {
        &[]
    } else {
        state.grown.as_slice()
    };
    supports.stand(grown, model, surface, bvh, transform, table);
    supports.set_raft_lift_mm(state.raft_lift_mm);
    supports
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};
    use core_supports::SupportPoint;
    use core_volume::HoleSize;

    use super::*;

    #[test]
    fn a_saved_tip_its_hole_drilled_out_is_not_brought_back() {
        let block = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let state = ObjectSupportState {
            points: [Vec3::new(5.0, 5.0, 0.0), Vec3::new(2.0, 2.0, 0.0)]
                .map(SupportPoint::new)
                .to_vec(),
            ..ObjectSupportState::default()
        };
        let hole = DrainHole::sized(Vec3::new(5.0, 5.0, 0.0), Vec3::Z, HoleSize::default(), 0.05);
        let bvh = Bvh::build(&block);
        let table = [SupportProfile::default()];

        let supports = supports_of(
            &state,
            &[hole],
            &block,
            &block,
            &bvh,
            Transform::default(),
            &table,
        );

        let contacts: Vec<Vec3> = supports
            .points()
            .iter()
            .map(|point| point.contact)
            .collect();
        assert_eq!(
            contacts,
            [Vec3::new(2.0, 2.0, 0.0)],
            "the tip in the 3 mm hole goes, the one 4 mm off it stays"
        );
    }
}
