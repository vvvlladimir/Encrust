use super::*;
use core_geometry::{Bvh, Mesh, Transform, cuboid};
use core_supports::{SupportPoint, columns, grow};
use printer_profiles::SupportProfile;

/// A slab whose underside at z = 20 runs over x 0..40, y −5..5, and `more` beside it.
fn slab_and(more: Option<Mesh>) -> Mesh {
    let mut model = cuboid(Vec3::new(0.0, -5.0, 20.0), Vec3::new(40.0, 5.0, 22.0));
    if let Some(more) = more {
        model.append(&more);
    }
    model
}

/// The supports grown under `model` at x = 5 and x = 35, too far apart to share a trunk, with
/// the columns they grew from.
fn grown_apart(
    model: &Mesh,
    profile: &SupportProfile,
) -> (Vec<SupportTree>, Vec<Column>, Vec<Vec3>) {
    let bvh = Bvh::build(model);
    let placed = Placed::new(model, &bvh, Transform::default());
    let points = [5.0, 35.0].map(|x| SupportPoint::new(Vec3::new(x, 0.0, 20.0)));
    let profiles = Profiles::single(profile);
    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    assert_eq!(trees.len(), 2, "30 mm apart, each stands on its own");
    (trees, columns, points.map(|point| point.contact).to_vec())
}

fn medium() -> SupportProfile {
    SupportProfile::shipped("medium").expect("the medium preset ships")
}

/// [`merged_near`] on `model` for the one merge `near`, both contacts where it is empty.
fn merge(model: &Mesh, near: Vec<Vec3>) -> Result<Vec<SupportTree>, EngineError> {
    let profile = medium();
    let (trees, columns, contacts) = grown_apart(model, &profile);
    let bvh = Bvh::build(model);
    let placed = Placed::new(model, &bvh, Transform::default());
    let near = if near.is_empty() {
        contacts.clone()
    } else {
        near
    };
    let profiles = Profiles::single(&profile);
    merged_near(trees, (&[near], &contacts), &columns, &placed, profiles)
}

#[test]
fn the_supports_named_by_their_contacts_stand_as_one_tree() {
    let trees = merge(&slab_and(None), Vec::new()).expect("clear air between them");
    assert_eq!(trees.len(), 1, "one trunk carries both");
    assert_eq!(trees[0].tip_count(), 2, "and holds both tips");
}

#[test]
fn a_point_near_no_contact_is_refused() {
    let far = Vec3::new(20.0, 0.0, 20.0);
    let refused = merge(&slab_and(None), vec![Vec3::new(5.0, 0.0, 20.0), far]);
    assert!(matches!(refused, Err(EngineError::NoSupportNear { at }) if at == far));
}

#[test]
fn two_points_on_one_support_have_nothing_to_merge() {
    let one = Vec3::new(5.0, 0.0, 20.0);
    let refused = merge(&slab_and(None), vec![one, one + Vec3::X * 0.1]);
    assert!(matches!(
        refused,
        Err(EngineError::OneSupportToMerge { .. })
    ));
}

#[test]
fn a_wall_between_the_supports_refuses_the_merge_and_says_why() {
    let wall = cuboid(Vec3::new(19.5, -5.0, 0.0), Vec3::new(20.5, 5.0, 20.0));
    let refused = merge(&slab_and(Some(wall)), Vec::new());
    assert!(matches!(
        refused,
        Err(EngineError::MergeRefused {
            why: Refused::Collides,
            ..
        })
    ));
}
