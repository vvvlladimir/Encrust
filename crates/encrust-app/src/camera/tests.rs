use super::*;

#[test]
fn the_view_matrix_puts_the_target_at_the_origin() {
    let camera = OrbitCamera {
        target: Vec3::new(10.0, 20.0, 5.0),
        ..OrbitCamera::default()
    };
    let seen = camera.view().transform_point3(camera.target);
    assert!(
        seen.abs_diff_eq(Vec3::new(0.0, 0.0, -camera.distance_mm), 1e-3),
        "the target sits on the view axis at the orbit distance, got {seen}"
    );
}

#[test]
fn the_eye_keeps_its_distance_from_the_target() {
    let mut camera = OrbitCamera::default();
    camera.orbit(1.3, -0.7);
    let distance = (camera.eye() - camera.target).length();
    assert!((distance - camera.distance_mm).abs() < 1e-3);
}

#[test]
fn orbiting_back_and_forth_is_the_identity() {
    let start = OrbitCamera::default();
    let mut camera = start;
    camera.orbit(0.9, 0.3);
    camera.orbit(-0.9, -0.3);
    assert!((camera.yaw_rad - start.yaw_rad).abs() < 1e-5);
    assert!((camera.pitch_rad - start.pitch_rad).abs() < 1e-5);
}

#[test]
fn pitch_stops_short_of_the_poles() {
    let mut camera = OrbitCamera::default();
    camera.orbit(0.0, 100.0);
    assert!(camera.pitch_rad <= PITCH_LIMIT_RAD);
    camera.orbit(0.0, -200.0);
    assert!(camera.pitch_rad >= -PITCH_LIMIT_RAD);
}

#[test]
fn zooming_in_never_reaches_the_target() {
    let mut camera = OrbitCamera::default();
    for _ in 0..200 {
        camera.zoom(0.5);
    }
    assert!(camera.distance_mm >= MIN_DISTANCE_MM);
}

#[test]
fn panning_moves_the_target_across_the_view_plane() {
    let mut camera = OrbitCamera::default();
    let before = camera.target;
    camera.pan(Vec2::new(30.0, 0.0), 600.0);

    let forward = (camera.target - camera.eye()).normalize();
    let moved = camera.target - before;
    assert!(moved.length() > 0.0, "a horizontal drag moves the target");
    assert!(
        moved.dot(forward).abs() < 1e-4,
        "panning does not change the orbit distance along the view axis"
    );
}

#[test]
fn a_framed_box_fits_inside_the_field_of_view() {
    let mut camera = OrbitCamera::default();
    let bounds = Aabb::new(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(5.0, 5.0, 20.0));
    camera.frame(&bounds);

    // Every corner has to land inside the clip volume once divided through by w.
    let view_projection = camera.view_projection(1.0);
    for corner in bounding_corners(&bounds) {
        let clip = view_projection * corner.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(
            ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
            "corner {corner} falls outside the view at {ndc}"
        );
    }
}

#[test]
fn the_opening_view_looks_at_the_middle_of_the_plate() {
    let plate = BuildPlate::default();
    let camera = OrbitCamera::framing_plate(&plate);
    assert_eq!(camera.target, plate.center());
    assert!(
        camera.eye().y < camera.target.y,
        "the default view is frontal"
    );
    assert!(camera.eye().z > camera.target.z, "and slightly from above");
}

fn bounding_corners(bounds: &Aabb) -> [Vec3; 8] {
    let (lo, hi) = (bounds.mins, bounds.maxs);
    [
        Vec3::new(lo.x, lo.y, lo.z),
        Vec3::new(hi.x, lo.y, lo.z),
        Vec3::new(lo.x, hi.y, lo.z),
        Vec3::new(hi.x, hi.y, lo.z),
        Vec3::new(lo.x, lo.y, hi.z),
        Vec3::new(hi.x, lo.y, hi.z),
        Vec3::new(lo.x, hi.y, hi.z),
        Vec3::new(hi.x, hi.y, hi.z),
    ]
}

#[test]
fn seen_from_the_front_the_eye_stands_in_front_of_the_target() {
    let camera = OrbitCamera::default().seen_from(Vec3::NEG_Y);
    let eye = camera.eye() - camera.target;
    assert!(
        eye.normalize().abs_diff_eq(Vec3::NEG_Y, 1e-5),
        "the eye is straight down -Y from the target, got {eye}"
    );
}

#[test]
fn seen_from_a_corner_the_eye_stands_on_its_diagonal() {
    let corner = Vec3::new(1.0, -1.0, 1.0);
    let camera = OrbitCamera::default().seen_from(corner);
    let eye = (camera.eye() - camera.target).normalize();
    assert!(eye.abs_diff_eq(corner.normalize(), 1e-5), "got {eye}");
}

#[test]
fn seen_from_above_keeps_the_yaw_and_stops_short_of_the_pole() {
    let start = OrbitCamera {
        yaw_rad: 0.7,
        ..OrbitCamera::default()
    };
    let camera = start.seen_from(Vec3::Z);
    assert_eq!(camera.yaw_rad, start.yaw_rad);
    assert_eq!(camera.pitch_rad, PITCH_LIMIT_RAD);
}

#[test]
fn a_turn_starts_where_it_was_and_ends_where_it_was_going() {
    let from = OrbitCamera::default();
    let to = OrbitCamera {
        target: Vec3::new(10.0, 0.0, 0.0),
        distance_mm: 120.0,
        ..from.seen_from(Vec3::X)
    };
    let turn = CameraTurn {
        from,
        to,
        started_s: 2.0,
        length_s: 0.5,
    };
    assert_eq!(turn.at(2.0), (from, false));
    let (end, over) = turn.at(2.5);
    assert!(over);
    assert!((end.yaw_rad - to.yaw_rad).abs() < 1e-5);
    assert!((end.distance_mm - to.distance_mm).abs() < 1e-3);
    assert!(end.target.abs_diff_eq(to.target, 1e-4));
}

#[test]
fn a_turn_takes_the_short_way_round() {
    let from = OrbitCamera {
        yaw_rad: PI - 0.1,
        ..OrbitCamera::default()
    };
    let to = OrbitCamera {
        yaw_rad: -PI + 0.1,
        ..from
    };
    let halfway = from.toward(&to, 0.5);
    assert!(
        (halfway.yaw_rad - PI).abs() < 1e-5,
        "the eye passes behind, not round the front, got {}",
        halfway.yaw_rad
    );
}

#[test]
fn without_perspective_a_framed_box_still_fits_the_view() {
    let mut camera = OrbitCamera {
        orthographic: true,
        ..OrbitCamera::default()
    };
    let bounds = Aabb::new(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(5.0, 5.0, 20.0));
    camera.frame(&bounds);
    let view_projection = camera.view_projection(1.0);
    for corner in bounding_corners(&bounds) {
        let clip = view_projection * corner.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(
            ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
            "corner {corner} falls outside the view at {ndc}"
        );
    }
}

/// Without perspective the view is as tall as the perspective one is at the target,
/// so a point standing there lands in the same place either way.
#[test]
fn switching_perspective_keeps_the_target_plane_the_same_size() {
    let perspective = OrbitCamera::default();
    let flat = OrbitCamera {
        orthographic: true,
        ..perspective
    };
    let up = perspective.view().inverse().transform_vector3(Vec3::Y);
    let point = perspective.target + up * 20.0;
    let seen = |camera: &OrbitCamera| {
        let clip = camera.view_projection(1.5) * point.extend(1.0);
        clip.truncate() / clip.w
    };
    assert!(
        (seen(&perspective).y - seen(&flat).y).abs() < 1e-4,
        "{} against {}",
        seen(&perspective),
        seen(&flat)
    );
}

#[test]
fn without_perspective_every_line_of_sight_runs_along_the_view_axis() {
    let camera = OrbitCamera {
        orthographic: true,
        ..OrbitCamera::default()
    };
    let point = camera.target + Vec3::new(30.0, 10.0, 5.0);
    let sight = (point - camera.sight_to(point)).normalize();
    let axis = (camera.target - camera.eye()).normalize();
    assert!(sight.abs_diff_eq(axis, 1e-5), "got {sight}");
    assert_eq!(
        OrbitCamera::default().sight_to(point),
        OrbitCamera::default().eye()
    );
}
