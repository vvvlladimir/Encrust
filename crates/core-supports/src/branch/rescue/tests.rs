use crate::Profiles;
use crate::SupportPoint;
use crate::column::has_head;
use crate::fixtures::{profile, slab_with_lip, trees_on};
use core_geometry::{Transform, Vec3};

#[test]
fn a_tip_by_a_lip_over_open_air_is_moved_along_its_face_to_where_a_head_fits() {
    // 0.1 mm short of the lip no head fits, turned or not, within the 0.3 mm a contact first
    // steps, and no small pillar has a part under it to stand on.
    let contact = Vec3::new(9.9, 0.0, 10.0);
    let table = [profile()];
    let profiles = Profiles::new(&table).expect("one profile");
    let points = [SupportPoint::new(contact)];
    let trees = trees_on(&points, &slab_with_lip(), Transform::default(), profiles);

    let tree = trees
        .iter()
        .find(|tree| tree.points().any(|point| point == 0))
        .expect("the tip is carried");
    let tip = tree
        .nodes()
        .iter()
        .find(|node| node.is_leaf())
        .expect("a tip");
    let neck = tree.nodes()[tip.parent.expect("a head under the tip")].position;
    assert!(
        has_head(tip.position, neck),
        "a head, not a bare strut: to {neck}"
    );
    let moved = tip.position - contact;
    assert!(
        moved.x < -0.3 && moved.length() <= 1.0 + 1e-4 && moved.z.abs() < 0.05 + 1e-4,
        "moved away from the lip, over 0.3 mm and within 1 mm at its height, by {moved}"
    );
}
