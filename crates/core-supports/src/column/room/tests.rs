use crate::column::has_head;
use crate::fixtures::{columns_on, profile, trees_on};
use crate::{Profiles, SupportPoint};
use core_geometry::{Mesh, Quat, Transform, Vec2, Vec3, prism, transform_mesh};
use printer_profiles::SupportProfile;

/// A barrel vault along y, its underside a half cylinder of radius 1 round x = 0, z = 6: under
/// the rim of a 1 mm tip at its crown the face rises 0.13 mm, past the head's hair and graze.
fn barrel_vault() -> Mesh {
    const STONES: usize = 96;
    let at = |stone: usize, radius: f32| {
        let angle = std::f32::consts::PI * stone as f32 / STONES as f32;
        Vec2::new(radius * angle.cos(), radius * angle.sin())
    };
    let upright = Transform {
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        translation: Vec3::new(0.0, 5.0, 6.0),
        ..Transform::default()
    };
    let mut model = Mesh::default();
    for stone in 0..STONES {
        let outline = [
            at(stone, 1.0),
            at(stone, 3.0),
            at(stone + 1, 3.0),
            at(stone + 1, 1.0),
        ];
        model.append(&transform_mesh(&prism(&outline, 0.0, 10.0), upright));
    }
    model
}

fn wide_tip() -> SupportProfile {
    let mut profile = profile();
    profile.tip.contact_diameter_mm = 1.0;
    profile.tip.contact_depth_mm = 0.0;
    profile
}

#[test]
fn a_tip_seated_on_a_barrel_vault_stands_on_a_head() {
    let model = barrel_vault();
    let profile = wide_tip();
    let points = [SupportPoint::new(Vec3::new(0.0, 0.0, 7.0))];
    let columns = columns_on(
        &points,
        &model,
        Transform::default(),
        Profiles::single(&profile),
    );
    let column = columns.first().expect("the vault leaves room for the head");
    assert!(
        has_head(column.contact, column.neck),
        "the face rising 0.13 mm under the rim is the tip's seat, not in the head's way; neck at {}",
        column.neck
    );
    let trees = trees_on(
        &points,
        &model,
        Transform::default(),
        Profiles::single(&profile),
    );
    assert_eq!(
        trees
            .iter()
            .map(crate::SupportTree::tip_count)
            .sum::<usize>(),
        1,
        "the support is drawn and stands"
    );
}
