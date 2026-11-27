use std::sync::Arc;

use core_edit::{Edit, ObjectId, OrientAim};
use core_engine::project::AutoSizing;
use core_geometry::{Transform, Vec2, Vec3, cuboid, prism, transform_mesh};
use core_volume::Cancel;
use printer_profiles::{Catalogue, ProfileError};

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

/// What the stood height may exceed the short side by, mm.
const HEIGHT_TOL_MM: f32 = 1e-3;

/// A plate of one 10 x 10 x 30 mm bar standing on its end.
fn standing_bar() -> Document {
    let mut project = cube();
    project.models[0].source = Arc::new(cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 30.0)));
    Document::from_project(project)
}

fn orient(goal: OrientAim, pick: usize) -> Edit {
    Edit::Orient {
        object: ObjectId(1),
        goal,
        max_tilt_deg: None,
        clean_surface: false,
        pick,
        plate_mm: None,
        low_mm: [0.0, 0.0],
        printer: None,
        resin: None,
        keep_clear_mm: Vec::new(),
        keep_clear_radius_mm: None,
        keep_clear_seen_deg: None,
        lift_mm: None,
    }
}

fn height_mm(document: &Document) -> f32 {
    let (state, meshes) = document
        .object(ObjectId(1))
        .expect("the bar is on the plate");
    let bounds = transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a bar has faces");
    bounds.maxs.z - bounds.mins.z
}

#[test]
fn a_bar_standing_on_its_end_is_laid_with_its_length_along_the_plate_for_the_shortest_print() {
    let mut document = standing_bar();
    let edit = orient(OrientAim::ShortestPrint, 0);
    assert!(height_mm(&document) > 29.0, "the bar starts 30 mm tall");

    let applied = apply(&mut document, &edit, Cancel::never()).expect("a bar can be oriented");

    assert!(matches!(applied.report, Report::Oriented { .. }));
    let height = height_mm(&document);
    assert!(
        height <= 10.0 + HEIGHT_TOL_MM,
        "the bar's 30 mm lies along the plate, so it stands 10 mm, got {height}"
    );
}

#[test]
fn a_watched_search_reports_how_far_it_has_got_to_the_end() {
    let mut document = standing_bar();
    let progress = core_volume::Progress::default();

    apply(
        &mut document,
        &orient(OrientAim::ShortestPrint, 0),
        Cancel::never().reporting(&progress),
    )
    .expect("a bar can be oriented");

    let (step, share) = progress.share().expect("the search reported its progress");
    assert_eq!(step, core_volume::Step::Searching);
    assert!(
        (share - 1.0).abs() < 1e-6,
        "a finished search is all done, got {share}"
    );
}

#[test]
fn a_pick_past_the_poses_found_is_no_choice() {
    let mut document = standing_bar();
    let edit = orient(OrientAim::ShortestPrint, 10_000);

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("no such pose");

    assert!(matches!(
        error,
        ApplyError::NoChoice { object: ObjectId(1), pick: 10_000, found } if found > 0
    ));
}

#[test]
fn a_cancelled_search_is_cancelled_and_leaves_the_bar_as_it_stood() {
    let mut document = standing_bar();
    let before = document
        .object(ObjectId(1))
        .expect("on the plate")
        .0
        .transform;
    let stop = || true;

    let error = apply(
        &mut document,
        &orient(OrientAim::Auto, 0),
        Cancel::when(&stop),
    )
    .expect_err("the search is stopped");

    assert_eq!(error, ApplyError::Cancelled);
    let after = document
        .object(ObjectId(1))
        .expect("on the plate")
        .0
        .transform;
    assert_eq!(after, before);
}

fn settings_for(document: &Document) -> OrientSettings {
    let (state, _) = document
        .object(ObjectId(1))
        .expect("the bar is on the plate");
    let plan = orient_plan_of(document, state)
        .expect("the sizing sizes")
        .expect("the plate has a group");
    placed_by(OrientSettings::default(), &plan)
}

fn keeping_sizing(on: bool) -> Document {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue reads");
    let mut profile = catalogue
        .autos()
        .next()
        .expect("one automatic profile ships")
        .profile
        .clone();
    profile.max_overhang_deg = 41.0;
    profile.contact_spacing_mm = 3.3;
    let mut project = cube();
    project.manifest.supports.sizing = Some(AutoSizing { on, profile });
    Document::from_project(project)
}

#[test]
fn a_plate_keeping_an_automatic_sizing_is_judged_by_its_numbers() {
    let sized = settings_for(&keeping_sizing(true));

    assert!(
        (sized.max_overhang_deg - 41.0).abs() < 1e-6,
        "overhang is the sizing's 41, got {}",
        sized.max_overhang_deg
    );
    assert!(
        (sized.contact_spacing_mm - 3.3).abs() < 1e-6,
        "spacing is the sizing's 3.3, got {}",
        sized.contact_spacing_mm
    );
}

#[test]
fn a_plate_without_a_sizing_in_use_is_judged_by_its_active_group() {
    let off = settings_for(&keeping_sizing(false));

    let project = cube();
    let group = &project.manifest.supports.groups[usize::from(project.manifest.supports.active)];
    let active = group.profile.clone();
    assert!(
        (off.max_overhang_deg - active.max_overhang_deg).abs() < 1e-6,
        "overhang is the group's {}, got {}",
        active.max_overhang_deg,
        off.max_overhang_deg
    );
    assert!(
        (off.contact_spacing_mm - active.contact_spacing_mm).abs() < 1e-6,
        "spacing is the group's {}, got {}",
        active.contact_spacing_mm,
        off.contact_spacing_mm
    );
}

#[test]
fn a_lift_asked_for_is_the_lift_the_search_judges_by() {
    let active = SupportProfile::shipped("medium").expect("the medium preset ships");
    let plan = orient_plan(None, &active, active.raft);

    let asked = placed_by(OrientSettings::default(), &lifted(plan.clone(), Some(3.0)));
    let unasked = placed_by(OrientSettings::default(), &lifted(plan, None));

    let raft_mm = if active.raft.enabled {
        active.raft.thickness_mm
    } else {
        0.0
    };
    assert!(
        (asked.lift_mm - (3.0 + raft_mm)).abs() < 1e-6,
        "a lift of 3 mm and the raft's {raft_mm}, got {}",
        asked.lift_mm
    );
    assert!(
        (unasked.lift_mm - active.lift_mm()).abs() < 1e-6,
        "unasked, the profile's {}, got {}",
        active.lift_mm(),
        unasked.lift_mm
    );
}

#[test]
fn a_kept_sizing_whose_shape_the_catalogue_lacks_names_that_shape() {
    let mut document = keeping_sizing(true);
    let mut project = cube();
    let sizing = document.support_state().sizing.clone().expect("kept");
    let mut profile = sizing.profile;
    profile.shapes[0] = "no-such-shape".to_owned();
    project.manifest.supports.sizing = Some(AutoSizing { on: true, profile });
    document = Document::from_project(project);

    let error = apply(&mut document, &orient(OrientAim::Auto, 0), Cancel::never())
        .expect_err("a sizing that cannot be sized is refused");

    assert_eq!(error, ApplyError::NoProfile("no-such-shape".to_owned()));
}

/// The box of the bar as it stands, as its low and high corners on the plate.
fn box_of(document: &Document) -> (Vec3, Vec3) {
    let (state, meshes) = document
        .object(ObjectId(1))
        .expect("the bar is on the plate");
    let bounds = transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a bar has faces");
    (bounds.mins, bounds.maxs)
}

/// The bar standing with the middle of its box at `middle` on the plate.
fn bar_at(middle: [f32; 2]) -> Document {
    let mut document = standing_bar();
    document.set_transform(
        ObjectId(1),
        Transform {
            translation: Vec3::new(middle[0] - 5.0, middle[1] - 5.0, 0.0),
            ..Transform::default()
        },
    );
    document
}

#[test]
fn an_orient_keeps_the_middle_of_the_box_over_the_spot_it_stood_over() {
    let mut document = bar_at([15.0, 15.0]);
    let edit = orient(OrientAim::ShortestPrint, 0);

    apply(&mut document, &edit, Cancel::never()).expect("a bar can be oriented");

    let (low, high) = box_of(&document);
    let middle = ((low + high) / 2.0).truncate();
    assert!(
        (middle - Vec2::new(15.0, 15.0)).abs().max_element() < 1e-3,
        "the box middle stays at (15, 15), got {middle}"
    );
}

#[test]
fn an_orient_that_overhangs_the_area_slides_onto_it() {
    let mut document = bar_at([35.0, 35.0]);
    let edit = Edit::Orient {
        object: ObjectId(1),
        goal: OrientAim::ShortestPrint,
        max_tilt_deg: None,
        clean_surface: false,
        pick: 0,
        plate_mm: Some([40.0, 40.0]),
        low_mm: [0.0, 0.0],
        printer: None,
        resin: None,
        keep_clear_mm: Vec::new(),
        keep_clear_radius_mm: None,
        keep_clear_seen_deg: None,
        lift_mm: None,
    };

    apply(&mut document, &edit, Cancel::never()).expect("a bar can be oriented");

    let (low, high) = box_of(&document);
    assert!(
        low.x > -1e-3 && low.y > -1e-3 && high.x < 40.0 + 1e-3 && high.y < 40.0 + 1e-3,
        "the 30 mm bar lies inside the 40 mm area, got {low} to {high}"
    );
}

fn orient_on(printer: Option<&str>, resin: Option<&str>) -> Edit {
    Edit::Orient {
        object: ObjectId(1),
        goal: OrientAim::ShortestPrint,
        max_tilt_deg: None,
        clean_surface: false,
        pick: 0,
        plate_mm: None,
        low_mm: [0.0, 0.0],
        printer: printer.map(str::to_owned),
        resin: resin.map(str::to_owned),
        keep_clear_mm: Vec::new(),
        keep_clear_radius_mm: None,
        keep_clear_seen_deg: None,
        lift_mm: None,
    }
}

#[test]
fn a_printer_the_catalogue_lacks_is_no_printer_and_leaves_the_bar_standing() {
    let mut document = standing_bar();

    let error = apply(
        &mut document,
        &orient_on(Some("no-such-printer"), None),
        Cancel::never(),
    )
    .expect_err("the printer is not listed");

    assert_eq!(error, ApplyError::NoPrinter("no-such-printer".to_owned()));
    assert!(
        height_mm(&document) > 29.0,
        "the bar still stands 30 mm tall"
    );
}

#[test]
fn a_resin_the_catalogue_lacks_is_no_resin() {
    let mut document = standing_bar();

    let error = apply(
        &mut document,
        &orient_on(None, Some("no-such-resin")),
        Cancel::never(),
    )
    .expect_err("the resin is not listed");

    assert_eq!(error, ApplyError::NoResin("no-such-resin".to_owned()));
}

#[test]
fn a_printer_named_in_the_catalogue_proves_the_poses_and_lays_the_bar_down() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue reads");
    let id = catalogue
        .printers()
        .next()
        .expect("a printer ships")
        .id
        .clone();
    let mut document = standing_bar();

    apply(&mut document, &orient_on(Some(&id), None), Cancel::never())
        .expect("a named printer proves the poses");

    let height = height_mm(&document);
    assert!(
        height <= 10.0 + HEIGHT_TOL_MM,
        "the bar's 30 mm lies along the plate, so it stands 10 mm, got {height}"
    );
}

fn asked_of(printer: Option<&str>) -> OrientAsked {
    OrientAsked {
        goal: OrientAim::ShortestPrint,
        max_tilt_deg: None,
        clean_surface: false,
        pick: 0,
        area_mm: None,
        printer: printer.map(str::to_owned),
        resin: None,
        keep_clear: (Vec::new(), None, None),
        lift_mm: None,
    }
}

#[test]
fn an_orient_asked_a_lift_stands_its_poses_on_supports_at_that_lift() {
    let document = Document::from_project(cube());
    let asked = OrientAsked {
        lift_mm: Some(3.0),
        ..asked_of(None)
    };

    let (_, lift_mm) =
        ranked_poses(&document, ObjectId(1), &asked, Cancel::never()).expect("a cube has poses");

    assert!(
        (lift_mm - 3.0).abs() < 1e-6,
        "the 3 mm asked, not the profile's, got {lift_mm}"
    );
}

#[test]
fn the_best_pose_is_printed_whether_a_printer_is_named_or_not() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue reads");
    let named = catalogue
        .printers()
        .next()
        .expect("a printer ships")
        .id
        .clone();
    let document = standing_bar();

    for printer in [None, Some(named.as_str())] {
        let (found, _) = ranked_poses(&document, ObjectId(1), &asked_of(printer), Cancel::never())
            .expect("the bar is searched");

        assert!(
            found[0].printout.is_some(),
            "printer {printer:?} proves the best pose"
        );
    }
}

#[test]
fn a_plate_with_no_support_group_proves_its_poses_on_the_default_group() {
    let mut project = cube();
    project.models[0].source = Arc::new(cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 30.0)));
    project.manifest.supports.groups.clear();
    let document = Document::from_project(project);

    let (found, _) = ranked_poses(&document, ObjectId(1), &asked_of(None), Cancel::never())
        .expect("the bar is searched");

    assert!(
        found[0].printout.is_some(),
        "the default group's plan is proved"
    );
}

#[test]
fn a_catalogue_that_cannot_be_read_is_its_own_error_not_a_missing_printer() {
    let broken = Err(ProfileError::BadId { id: "x".to_owned() });

    let error = machine_in(broken, (None, None)).expect_err("no catalogue, no machine");

    assert!(matches!(error, ApplyError::BadCatalogue(_)), "{error:?}");
}

#[test]
fn a_body_whose_centre_of_mass_is_off_its_box_middle_keeps_the_box_middle() {
    let legs = [Vec2::ZERO, Vec2::new(30.0, 0.0), Vec2::new(0.0, 10.0)];
    let mut project = cube();
    project.models[0].source = Arc::new(prism(&legs, 0.0, 30.0));
    let mut document = Document::from_project(project);
    document.set_transform(
        ObjectId(1),
        Transform {
            translation: Vec3::new(35.0, 45.0, 0.0),
            ..Transform::default()
        },
    );
    let edit = orient(OrientAim::ShortestPrint, 0);

    apply(&mut document, &edit, Cancel::never()).expect("a prism can be oriented");

    let (low, high) = box_of(&document);
    let middle = ((low + high) / 2.0).truncate();
    assert!(
        (middle - Vec2::new(50.0, 50.0)).abs().max_element() < 1e-3,
        "the box middle stays at (50, 50), got {middle}"
    );
    assert!(high.z - low.z < 10.0 + HEIGHT_TOL_MM, "the prism lies down");
}

#[test]
fn a_place_kept_clear_on_a_moved_object_is_read_in_its_own_frame() {
    let cube = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let standing = Transform::from_translation(Vec3::new(20.0, 0.0, 0.0));
    let points = [Vec3::new(25.0, 5.0, 10.0)];

    let kept = kept_clear(
        &cube,
        standing,
        (&points, None, None),
        Vec2::new(100.0, 100.0),
    )
    .expect("a place asked is kept clear");

    assert!(
        kept.at_mm[0].abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-5),
        "the top middle of the cube, (5, 5, 10), got {:?}",
        kept.at_mm[0]
    );
    assert!(
        (kept.radius_mm - core_engine::KEEP_CLEAR_RADIUS_MM).abs() < 1e-9,
        "the default reach, {} mm",
        core_engine::KEEP_CLEAR_RADIUS_MM
    );
}
