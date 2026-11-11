use super::*;
use crate::fixtures::{box_mesh, profile, under_a_gap_over_a_bar};
use crate::{columns, grow};
use core_geometry::{Bvh, Mesh, Transform, Vec3};
use printer_profiles::SupportProfile;

/// What is lost of one point at `contact` on `model`, grown to `profile`.
fn lost_of(model: &Mesh, contact: Vec3, profile: SupportProfile) -> Vec<Lost> {
    let bvh = Bvh::build(model);
    let placed = Placed::new(model, &bvh, Transform::default());
    let table = [profile];
    let profiles = Profiles::new(&table).expect("one profile");
    let points = [SupportPoint::new(contact)];
    let trees = grow(&columns(&points, &placed, profiles), &placed, profiles);
    lost(&points, &trees, &placed, profiles)
}

#[test]
fn a_support_in_open_air_is_not_lost() {
    let model = box_mesh(Vec3::new(-5.0, -5.0, 10.0), Vec3::new(5.0, 5.0, 12.0));
    assert!(lost_of(&model, Vec3::new(0.0, 0.0, 10.0), profile()).is_empty());
}

#[test]
fn a_head_running_into_the_model_is_lost_as_head_blocked() {
    // Half a millimetre of air over a bar, where a 2 mm head runs into it.
    let mut plain = profile();
    plain.small_pillar.enabled = false;
    let lost = lost_of(&under_a_gap_over_a_bar(), Vec3::new(0.0, 0.0, 10.5), plain);
    assert_eq!(
        lost,
        [Lost {
            point: 0,
            why: Unstood::HeadBlocked
        }]
    );
}

#[test]
fn a_tip_on_a_face_looking_up_is_lost_as_faces_up() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let lost = lost_of(&model, Vec3::new(5.0, 5.0, 10.0), profile());
    assert_eq!(
        lost,
        [Lost {
            point: 0,
            why: Unstood::FacesUp
        }]
    );
}

#[test]
fn a_tip_over_a_floor_it_may_not_stand_on_is_lost_as_no_ground() {
    // A table 10 mm over a floor wider than the 25 mm search reaches, the floor barred.
    let mut model = box_mesh(Vec3::new(-50.0, -50.0, 15.0), Vec3::new(50.0, 50.0, 17.0));
    model.append(&box_mesh(
        Vec3::new(-50.0, -50.0, 0.0),
        Vec3::new(50.0, 50.0, 5.0),
    ));
    let mut barred = profile();
    barred.land_on_model = false;
    let lost = lost_of(&model, Vec3::new(0.0, 0.0, 15.0), barred);
    assert_eq!(
        lost,
        [Lost {
            point: 0,
            why: Unstood::NoGround
        }]
    );
}
