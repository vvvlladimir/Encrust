use std::sync::Arc;

use super::*;

#[test]
fn a_hidden_support_is_not_in_the_way_of_the_one_behind_it() {
    let table = vec![printer_profiles::SupportProfile::default(); 2];
    let mut supports = core_supports::ModelSupports::default();
    supports.add(
        Vec3::new(0.0, 0.0, 20.0),
        core_geometry::Transform::default(),
        0,
    );
    supports.add(
        Vec3::new(30.0, 0.0, 20.0),
        core_geometry::Transform::default(),
        1,
    );
    let nothing = core_geometry::Mesh::default();
    let bvh = core_geometry::Bvh::build(&nothing);
    supports.refresh(&nothing, &bvh, core_geometry::Transform::default(), &table);
    let trees = supports.trees();
    let profiles = Profiles::new(&table).expect("two profiles");
    // Level with the middle of both pillars, the first group's nearer the eye.
    let ray = Ray::new(Vec3::new(-50.0, 0.0, 10.0), Vec3::X);
    let group_met = |hidden: &[bool]| {
        grab_shown(trees, hidden, &ray, profiles, Slack::default())
            .map(|(hit, _)| trees[hit.tree].group())
    };

    assert_eq!(group_met(&[false, false]), Some(0), "the nearer one");
    assert_eq!(group_met(&[true, false]), Some(1), "the one behind it");
    assert_eq!(group_met(&[true, true]), None);
}

/// BUG: the cap was no surface, so the zoom closed on the plate under it and a notch
/// carried the eye through the cap the cursor was on.
#[test]
fn a_scroll_over_a_cut_zooms_towards_its_cap() {
    let mut doc = crate::state::Doc::default();
    let (low, high) = (Vec3::new(50.0, 20.0, 0.0), Vec3::new(100.0, 60.0, 20.0));
    doc.scene = crate::scene::fixtures::scene_of_boxes(&[(low, high)]);
    let mut parts = crate::panels::fixtures::Parts::of(doc, crate::workspace::Tool::Position);
    parts.view.section.height_mm = Some(10.0);
    parts.view.camera.target = Vec3::new(75.0, 40.0, 10.0);
    parts.view.camera.distance_mm = 200.0;
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

    let anchor = zoom_anchor(&parts.window(), viewport, viewport.center())
        .expect("the middle of the view is on the cut box");
    assert!(
        anchor.abs_diff_eq(Vec3::new(75.0, 40.0, 10.0), 1e-2),
        "the cap at 10 mm where the camera looks, got {anchor}"
    );
}

#[test]
fn a_click_on_a_support_measures_from_it_and_the_next_from_the_model() {
    let mut doc = crate::state::Doc::default();
    let (low, high) = (Vec3::new(40.0, 40.0, 10.0), Vec3::new(60.0, 60.0, 20.0));
    doc.scene = crate::scene::fixtures::scene_of_boxes(&[(low, high)]);
    let mut parts = crate::panels::fixtures::Parts::of(doc, crate::workspace::Tool::Measure);
    let table = parts
        .tools
        .supports
        .table_for(&parts.doc.scene.objects()[0]);
    let object = &mut parts.doc.scene.objects_mut()[0];
    object
        .supports
        .add(Vec3::new(50.0, 50.0, 10.0), object.transform, 0);
    let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
    object
        .supports
        .refresh(&mesh, &bvh, object.transform, &table);
    // Level with the middle of the pillar under the box.
    let camera = &mut parts.view.camera;
    (camera.target, camera.pitch_rad, camera.distance_mm) =
        (Vec3::new(50.0, 50.0, 5.0), 0.0, 200.0);
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

    measure_under_cursor(&mut parts.window(), viewport, viewport.center(), false);
    // 30 points up is 8.5 mm at 200 mm: the box's near side at z 13.5 mm.
    let on_box = viewport.center() - egui::vec2(0.0, 30.0);
    measure_under_cursor(&mut parts.window(), viewport, on_box, false);

    let (from, to) = parts.tools.measure.span().expect("both ends are picked");
    assert!(
        from.abs_diff_eq(Vec3::new(50.0, 50.0, 5.0), 1e-2),
        "the pillar's axis at 5 mm where the cursor crosses it, got {from}"
    );
    assert!(
        to.z >= 10.0 && to.z <= 20.0 && (to.y - 40.0).abs() < 1e-2,
        "the near side of the box above it, got {to}"
    );
}

#[test]
fn the_aim_reaches_as_far_on_screen_wherever_the_cursor_is_and_however_deep() {
    let camera = OrbitCamera::default();
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

    // 600 points of height show 2 * 300 mm * tan(0.4) = 254 mm, so a point is
    // 0.42 mm where the camera looks, six of them 2.54 mm, and twice as far 5.08 mm.
    let slack = aim_slack(&camera, viewport, viewport.center());
    let looked_at = camera.distance_mm;
    let middle = slack.at(looked_at);
    assert!(
        (middle - 2.54).abs() < 0.1,
        "six points at the middle of the view are 2.54 mm, got {middle}"
    );
    let twice = slack.at(2.0 * looked_at);
    assert!(
        (twice - 2.0 * middle).abs() < 0.1,
        "twice as far off they are twice as many millimetres, got {twice}"
    );
    let corner = aim_slack(&camera, viewport, egui::pos2(40.0, 40.0)).at(looked_at);
    assert!(
        corner < 2.0 * middle,
        "a cursor far from the middle reaches no further, got {corner} mm"
    );
}

#[test]
fn a_click_outside_the_selection_only_aims_the_tool() {
    let mut scene = Scene::default();
    let cube = Arc::new(core_geometry::Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        vec![[0, 1, 2]],
    ));
    let summary = crate::scene::ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: core_geometry::Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: core_geometry::diagnose(&cube),
    };
    let first = scene.insert(crate::scene::Imported::new(
        "first".to_owned(),
        Arc::clone(&cube),
        core_geometry::Transform::default(),
        summary.clone(),
    ));
    let second = scene.insert(crate::scene::Imported::new(
        "second".to_owned(),
        cube,
        core_geometry::Transform::default(),
        summary,
    ));

    scene.select(Some(first));
    assert!(
        aims_elsewhere(&scene, Some(second)),
        "the click lands on a model the tool is not working on"
    );
    assert!(
        !aims_elsewhere(&scene, Some(first)),
        "a second click on what is picked is the tool's"
    );
    assert!(
        !aims_elsewhere(&scene, None),
        "a click on empty plate is not an aim"
    );

    scene.clear_selection();
    assert!(
        !aims_elsewhere(&scene, Some(second)),
        "with nothing picked the tool has the whole plate"
    );
}
