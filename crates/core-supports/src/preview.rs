use core_geometry::{Mesh, Vec3};

use crate::column::{column, square_head};
use crate::group::Profiles;
use crate::mesher::{mesh_alone, mesh_head};
use crate::placed::Placed;
use crate::{SupportTree, grow};

/// A support shown where the hand is about to put one, before it is put down.
#[derive(Debug, Clone)]
pub struct Preview {
    /// The support as it would be built standing alone, or only its tip when it cannot
    /// stand; in plate coordinates.
    pub mesh: Mesh,
    /// Whether a support put down here is built at all.
    pub stands: bool,
}

/// What a support of `group` touching the model at `contact` would be built as.
///
/// It is grown by the same rule a placed point is (ADR 0040), but alone: what it would
/// merge with or be braced to among the supports already standing is left out, so the
/// rebuild a hover asks for stays one column. `contact` is in plate coordinates.
pub fn preview(placed: &Placed, contact: Vec3, group: u16, profiles: Profiles) -> Preview {
    if let Some(tree) = alone(placed, contact, group, profiles) {
        return Preview {
            mesh: mesh_alone(&tree, profiles, placed),
            stands: true,
        };
    }
    let profile = profiles.of(group);
    let (contact, neck) = square_head(placed, contact, profile);
    Preview {
        mesh: mesh_head(contact, neck, profile),
        stands: false,
    }
}

/// Whether a support of `group` put down at `contact` alone is built at all, which is
/// what a hand placing one is refused by. `contact` is in plate coordinates.
pub fn stands(placed: &Placed, contact: Vec3, group: u16, profiles: Profiles) -> bool {
    alone(placed, contact, group, profiles).is_some()
}

/// The tree a lone support at `contact` grows into, if it stands.
fn alone(placed: &Placed, contact: Vec3, group: u16, profiles: Profiles) -> Option<SupportTree> {
    let column = column(placed, 0, group, contact, profiles.of(group))?;
    grow(&[column], placed, profiles)
        .into_iter()
        .find(|tree| tree.points().next().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{box_mesh, profile};
    use core_geometry::{Bvh, Transform};

    #[test]
    fn a_contact_under_a_floating_box_stands_on_the_plate() {
        let model = box_mesh(Vec3::new(-2.0, -2.0, 10.0), Vec3::new(2.0, 2.0, 12.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let fixture = profile();

        let shown = preview(
            &placed,
            Vec3::new(0.0, 0.0, 10.0),
            0,
            Profiles::single(&fixture),
        );

        assert!(shown.stands, "the plate is clear under the box");
        assert!(stands(
            &placed,
            Vec3::new(0.0, 0.0, 10.0),
            0,
            Profiles::single(&fixture)
        ));
        let lowest = shown
            .mesh
            .vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(f32::INFINITY, f32::min);
        assert!(
            lowest.abs() < 1e-3,
            "a support standing on the plate reaches it, got {lowest}"
        );
    }

    #[test]
    fn a_contact_over_the_model_is_only_a_tip_when_the_model_may_not_be_stood_on() {
        let mut model = box_mesh(Vec3::new(-20.0, -20.0, 10.0), Vec3::new(20.0, 20.0, 12.0));
        let floor = box_mesh(Vec3::new(-20.0, -20.0, 0.0), Vec3::new(20.0, 20.0, 5.0));
        let offset = model.vertices.len() as u32;
        model.vertices.extend(floor.vertices);
        model.faces.extend(
            floor
                .faces
                .iter()
                .map(|face| face.map(|vertex| vertex + offset)),
        );
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let mut fixture = profile();
        fixture.land_on_model = false;

        let shown = preview(
            &placed,
            Vec3::new(0.0, 0.0, 10.0),
            0,
            Profiles::single(&fixture),
        );

        assert!(
            !shown.stands,
            "the lower slab is under it and too wide to lean past"
        );
        assert!(!shown.mesh.faces.is_empty(), "the tip is still shown");
        assert!(!stands(
            &placed,
            Vec3::new(0.0, 0.0, 10.0),
            0,
            Profiles::single(&fixture)
        ));
        let lowest = shown
            .mesh
            .vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(f32::INFINITY, f32::min);
        assert!(
            lowest > 7.0,
            "only the 2 mm tip under the contact at 10 mm is shown, got {lowest}"
        );
    }
}
