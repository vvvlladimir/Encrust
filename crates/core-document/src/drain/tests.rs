use std::sync::Arc;

use core_edit::Edit;
use core_engine::project::{HollowState, load};
use core_engine::{Baked, trapped_in};
use core_geometry::{Aabb, Mesh, Transform, transform_mesh};
use core_supports::SupportPoint;
use core_volume::{Cancel, HollowMode, InfillSettings, PLUG_SPACING_MM};

use super::*;
use crate::apply::apply;
use crate::fixtures::{cube, cube_of};
use crate::piece::Role;

const CUBE: ObjectId = ObjectId(1);

fn drain_at(at_mm: Vec3) -> Edit {
    Edit::Drain {
        object: CUBE,
        at_mm: vec![at_mm],
        size: HoleSize::default(),
        kind: HoleKind::default(),
        auto: false,
        vent: false,
    }
}

#[test]
fn a_point_over_the_top_face_drills_a_hole_down_into_it() {
    let mut document = Document::from_project(cube());

    let applied = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect("a point over the top face drains");

    assert_eq!(
        applied.report,
        Report::Drained {
            holes: 1,
            trapped: 0,
            searched: None,
        }
    );
    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [hole] = state.hollow.drains[..] else {
        panic!("one hole was drilled, got {:?}", state.hollow.drains);
    };
    assert!(
        hole.at.abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-3),
        "the mouth is at (5, 5, 10) within 1e-3, got {}",
        hole.at
    );
    let axis = hole.axis.normalize();
    assert!(
        axis.abs_diff_eq(Vec3::NEG_Z, 1e-4),
        "the axis points down (0, 0, -1) within 1e-4, got {axis}"
    );
}

#[test]
fn a_point_over_a_corner_is_off_a_face() {
    let mut document = Document::from_project(cube());

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(10.5, 10.5, 10.5)),
        Cancel::never(),
    )
    .expect_err("a corner is no face to drill into");

    assert!(matches!(error, ApplyError::OffAFace { object, .. } if object == CUBE));
    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert!(
        state.hollow.drains.is_empty(),
        "the plate is left as it was"
    );
}

#[test]
fn a_model_with_no_faces_has_no_surface_to_drain() {
    let mut project = cube();
    project.models[0].source = Arc::new(Mesh::default());
    let mut document = Document::from_project(project);

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect_err("nothing to drill into");

    assert_eq!(error, ApplyError::NoSurface(CUBE));
}

#[test]
fn a_drained_plate_written_and_read_back_keeps_the_hole() {
    let mut document = Document::from_project(cube());
    apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect("the cube drains");
    let path = std::env::temp_dir().join(format!("{}-drained.encrust", std::process::id()));

    core_engine::project::save(&path, &document.into_project()).expect("the plate is written");
    let read = load(&path).expect("the plate reads back");
    std::fs::remove_file(&path).expect("the file is there to remove");

    let [hole] = read.manifest.objects[0].hollow.drains[..] else {
        panic!(
            "one hole read back, got {:?}",
            read.manifest.objects[0].hollow.drains
        );
    };
    assert!(
        hole.at.abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-3),
        "the mouth is at (5, 5, 10) within 1e-3, got {}",
        hole.at
    );
}

#[test]
fn a_transform_that_cannot_be_inverted_is_bad_and_leaves_the_cube() {
    let mut project = cube();
    project.manifest.objects[0].transform.scale = Vec3::new(0.0, 1.0, 1.0);
    let mut document = Document::from_project(project);

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect_err("a flat object has no inverse");

    assert_eq!(error, ApplyError::BadTransform(CUBE));
}

#[test]
fn a_hole_on_a_scaled_object_prints_at_its_size_on_the_plate() {
    let mut project = cube();
    project.manifest.objects[0].transform.scale = Vec3::splat(2.0);
    let mut document = Document::from_project(project);
    let edit = Edit::Drain {
        object: CUBE,
        at_mm: vec![Vec3::new(10.0, 10.0, 20.0)],
        size: HoleSize {
            diameter_mm: 3.0,
            ..HoleSize::default()
        },
        kind: HoleKind::default(),
        auto: false,
        vent: false,
    };

    apply(&mut document, &edit, Cancel::never()).expect("the top of the scaled cube drains");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [hole] = state.hollow.drains[..] else {
        panic!("one hole was drilled, got {:?}", state.hollow.drains);
    };
    let on_plate_mm = hole.diameter_mm * state.transform.least_scale();
    assert!(
        (on_plate_mm - 3.0).abs() < 1e-4,
        "the hole measures 3 mm on the plate, got {on_plate_mm}"
    );
}

#[test]
fn a_hole_takes_away_the_supports_it_drills_out_and_leaves_the_rest() {
    let mut project = cube();
    let object = &mut project.manifest.objects[0];
    object.transform = Transform::from_translation(Vec3::new(0.0, 0.0, 5.0));
    object.supports.points = vec![
        SupportPoint::new(Vec3::new(5.0, 5.0, 0.0)),
        SupportPoint::new(Vec3::new(1.0, 1.0, 0.0)),
    ];
    let mut document = Document::from_project(project);
    let edit = Edit::Drain {
        object: CUBE,
        at_mm: vec![Vec3::new(5.0, 5.0, 5.0)],
        size: HoleSize {
            diameter_mm: 3.0,
            ..HoleSize::default()
        },
        kind: HoleKind::default(),
        auto: false,
        vent: false,
    };

    apply(&mut document, &edit, Cancel::never()).expect("the underside of the cube drains");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let kept: Vec<Vec3> = state.supports.points.iter().map(|p| p.contact).collect();
    assert_eq!(
        kept,
        [Vec3::new(1.0, 1.0, 0.0)],
        "the contact under the hole goes, the one 4 mm off stays"
    );
}

fn hollowed_cube() -> Document {
    let mut document = Document::from_project(cube_of(20.0));
    let walled = Edit::Hollow {
        object: CUBE,
        settings: HollowState {
            thickness_mm: 2.0,
            mode: HollowMode::Internal,
            precision: 1.0,
            infill_on: false,
            infill: InfillSettings::default(),
            open_floor: false,
            solid_under_mm: None,
        },
        keep_solid: Vec::new(),
        keep_solid_reach_mm: None,
    };
    apply(&mut document, &walled, Cancel::never()).expect("a 20 mm cube takes a 2 mm wall");
    document
}

#[test]
fn a_hole_set_by_hand_into_a_hollow_cube_stops_at_its_depth() {
    let mut document = hollowed_cube();
    let edit = Edit::Drain {
        object: CUBE,
        at_mm: vec![Vec3::new(10.0, 10.0, 20.5)],
        size: HoleSize::default(),
        kind: HoleKind::AtDepth,
        auto: false,
        vent: false,
    };

    apply(&mut document, &edit, Cancel::never()).expect("a hole drills into the top");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [hole] = state.hollow.drains[..] else {
        panic!("one hole was drilled, got {:?}", state.hollow.drains);
    };
    assert_eq!(hole.kind, HoleKind::AtDepth, "a hole set by hand");
    let depth_mm = HoleSize::default().depth_mm;
    assert!(
        (hole.depth_mm - depth_mm).abs() < 1e-4,
        "it stops at {depth_mm} mm, got {}",
        hole.depth_mm
    );
}

fn searching() -> Edit {
    Edit::Drain {
        object: CUBE,
        at_mm: Vec::new(),
        size: HoleSize::default(),
        kind: HoleKind::default(),
        auto: true,
        vent: true,
    }
}

#[test]
fn a_search_drains_a_hollow_cube_through_holes_open_on_its_surface() {
    let mut document = hollowed_cube();

    let applied = apply(&mut document, &searching(), Cancel::never())
        .expect("a hollow cube has a cavity to drain");

    assert!(
        matches!(applied.report, Report::Drained { holes, trapped: 0, searched: Some(_) } if holes >= 1),
        "at least one hole and no pocket left trapped, got {:?}",
        applied.report
    );
    let (state, meshes) = document.object(CUBE).expect("the cube stays");
    for hole in &state.hollow.drains {
        let on_surface = [hole.at.x, hole.at.y, hole.at.z]
            .iter()
            .any(|axis| axis.abs() < 1e-3 || (axis - 20.0).abs() < 1e-3);
        assert!(
            on_surface,
            "a mouth is on a face of the cube within 1e-3, got {}",
            hole.at
        );
    }
    let mut solid = meshes.printed().as_ref().clone();
    let cuts = hollow_of(&state.hollow, meshes);
    solid.append(&transform_mesh(
        cuts.cut_bodies().expect("the holes are cut"),
        state.transform,
    ));
    let baked = Baked {
        solid: solid.into(),
        ceiling_mm: 20.0,
        floor_mm: 0.0,
    };
    let left = trapped_in(&baked, 0.05, |_, _| true).expect("the cube slices");
    assert_eq!(left, Some(Vec::new()), "no resin is left trapped");
}

#[test]
fn a_search_on_a_solid_cube_has_no_cavity_to_drain() {
    let mut document = Document::from_project(cube());

    let error = apply(&mut document, &searching(), Cancel::never())
        .expect_err("a solid cube traps nothing");

    assert_eq!(error, ApplyError::NoCavity(CUBE));
}

#[test]
fn a_search_given_up_is_cancelled_and_leaves_the_cube_undrilled() {
    let mut document = hollowed_cube();

    let stopped = || true;
    let error = apply(&mut document, &searching(), Cancel::when(&stopped))
        .expect_err("a stopped search lands nothing");

    assert_eq!(error, ApplyError::Cancelled);
    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert!(state.hollow.drains.is_empty());
}

#[test]
fn a_channel_adds_one_channel_of_the_points_given() {
    let mut document = Document::from_project(cube());
    let points_mm = vec![Vec3::new(2.0, 5.0, 5.0), Vec3::new(8.0, 5.0, 5.0)];
    let edit = Edit::Channel {
        object: CUBE,
        points_mm: points_mm.clone(),
        diameter_mm: 2.0,
    };

    let applied = apply(&mut document, &edit, Cancel::never()).expect("a channel is dug");

    assert_eq!(applied.report, Report::Channelled);
    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [channel] = &state.hollow.channels[..] else {
        panic!("one channel, got {:?}", state.hollow.channels);
    };
    assert_eq!(channel.points, points_mm);
    assert!((channel.diameter_mm - 2.0).abs() < 1e-6);
}

#[test]
fn plugs_for_two_holes_are_two_objects_naming_the_cube() {
    let mut document = Document::from_project(cube());
    let mut holes = drain_at(Vec3::new(3.0, 5.0, 10.5));
    if let Edit::Drain { at_mm, .. } = &mut holes {
        at_mm.push(Vec3::new(7.0, 5.0, 10.5));
    }
    apply(&mut document, &holes, Cancel::never()).expect("two holes drill");

    let edit = plugs_edit();
    let applied = apply(&mut document, &edit, Cancel::never()).expect("the plugs are cut");

    assert_eq!(
        applied.report,
        Report::Plugged {
            plugs: 2,
            closed: 0,
            crowded: 0
        }
    );
    let roles: Vec<Role> = applied.added.iter().map(|(_, role)| *role).collect();
    assert_eq!(roles, [Role::Plug(1), Role::Plug(2)]);
    for (plug, _) in &applied.added {
        let (state, meshes) = document.object(*plug).expect("a plug stands on the plate");
        assert_eq!(
            state.plug_of,
            Some(0),
            "a plug names the cube, the first object"
        );
        assert!(!meshes.source.is_empty());
    }
    let again = apply(&mut document, &edit, Cancel::never()).expect("laid again");
    assert_eq!(
        again.added.len(),
        2,
        "plugs laid again replace those before"
    );
    let before: Vec<ObjectId> = applied.added.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        again.removed, before,
        "the plugs laid before are reported gone"
    );
    assert!(document.object(before[0]).is_none());
}

fn plugs_edit() -> Edit {
    Edit::Plugs {
        object: CUBE,
        gap_mm: 0.1,
        plate_mm: None,
        low_mm: [0.0, 0.0],
    }
}

fn drained_cube() -> Document {
    let mut document = Document::from_project(cube());
    apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect("the cube drains");
    document
}

/// A drained cube with a second cube at x 14 to 24, where a row of plugs would stand.
fn drained_cube_with_neighbour() -> Document {
    let mut project = cube();
    let (mut state, meshes) = (
        project.manifest.objects[0].clone(),
        project.models[0].clone(),
    );
    state.id = ObjectId(2);
    state.transform = Transform::from_translation(Vec3::new(14.0, 0.0, 0.0));
    project.manifest.objects.push(state);
    project.models.push(meshes);
    let mut document = Document::from_project(project);
    let mut holes = drain_at(Vec3::new(3.0, 5.0, 10.5));
    if let Edit::Drain { at_mm, .. } = &mut holes {
        at_mm.push(Vec3::new(7.0, 5.0, 10.5));
    }
    apply(&mut document, &holes, Cancel::never()).expect("two holes drill");
    document
}

fn plug_boxes(document: &Document, applied: &Applied) -> Vec<Aabb> {
    (applied.added.iter())
        .map(|(id, _)| {
            let (_, meshes) = document.object(*id).expect("a plug stands on the plate");
            meshes.source.aabb().expect("a plug has faces")
        })
        .collect()
}

fn apart_mm(a: &Aabb, b: &Aabb) -> Scalar {
    let x = (a.mins.x - b.maxs.x).max(b.mins.x - a.maxs.x);
    let y = (a.mins.y - b.maxs.y).max(b.mins.y - a.maxs.y);
    x.max(y)
}

#[test]
fn plugs_with_an_area_stand_clear_of_the_models_and_each_other_inside_it() {
    let mut document = drained_cube_with_neighbour();
    let edit = Edit::Plugs {
        object: CUBE,
        gap_mm: 0.1,
        plate_mm: Some([100.0, 100.0]),
        low_mm: [0.0, 0.0],
    };

    let applied = apply(&mut document, &edit, Cancel::never()).expect("the plugs are cut");

    let plugs = plug_boxes(&document, &applied);
    assert_eq!(plugs.len(), 2, "one plug a hole");
    let mut others = vec![
        bounds_of(&document, ObjectId(1)),
        bounds_of(&document, ObjectId(2)),
    ];
    for (index, plug) in plugs.iter().enumerate() {
        assert!(
            plug.mins.x >= -1e-3
                && plug.mins.y >= -1e-3
                && plug.maxs.x <= 100.0 + 1e-3
                && plug.maxs.y <= 100.0 + 1e-3,
            "plug {index} is inside the 100 mm area, got {plug:?}"
        );
        for other in &others {
            let apart = apart_mm(plug, other);
            assert!(
                apart >= PLUG_SPACING_MM - 1e-3,
                "plug {index} is at least {PLUG_SPACING_MM} mm from every box, got {apart}"
            );
        }
        others.push(*plug);
    }
}

#[test]
fn plugs_with_no_area_stand_in_the_row_past_the_model() {
    let mut document = drained_cube_with_neighbour();

    let applied = apply(&mut document, &plugs_edit(), Cancel::never()).expect("the plugs are cut");

    let plugs = plug_boxes(&document, &applied);
    let expected = 10.0 + PLUG_SPACING_MM;
    assert!(
        (plugs[0].mins.x - expected).abs() < 1e-3,
        "the first plug starts {PLUG_SPACING_MM} mm past the cube at x = {expected}, got {}",
        plugs[0].mins.x
    );
}

#[test]
fn plugs_that_fit_nowhere_on_a_plate_are_counted_crowded() {
    let mut document = drained_cube_with_neighbour();
    let edit = Edit::Plugs {
        object: CUBE,
        gap_mm: 0.1,
        plate_mm: Some([30.0, 12.0]),
        low_mm: [0.0, 0.0],
    };

    let applied = apply(&mut document, &edit, Cancel::never()).expect("the plugs are cut");

    let Report::Plugged { plugs, crowded, .. } = applied.report else {
        panic!("plugs were laid, got {:?}", applied.report);
    };
    assert_eq!((plugs, crowded), (2, 2), "both plugs found no room");
}

fn bounds_of(document: &Document, id: ObjectId) -> Aabb {
    let (state, meshes) = document.object(id).expect("it is on the plate");
    transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a cube has faces")
}

#[test]
fn plugs_given_up_are_cancelled() {
    let mut document = drained_cube();

    let stopped = || true;
    let error = apply(&mut document, &plugs_edit(), Cancel::when(&stopped))
        .expect_err("a stopped cut lands nothing");

    assert_eq!(error, ApplyError::Cancelled);
}

#[test]
fn plugs_of_a_model_with_no_faces_have_no_surface_to_stand_beside() {
    let mut project = drained_cube().into_project();
    project.models[0].source = Arc::new(Mesh::default());
    let mut document = Document::from_project(project);

    let error =
        apply(&mut document, &plugs_edit(), Cancel::never()).expect_err("nothing to stand beside");

    assert_eq!(error, ApplyError::NoSurface(CUBE));
}

#[test]
fn a_channel_of_one_point_cannot_be_drilled() {
    let mut document = Document::from_project(cube());
    let edit = Edit::Channel {
        object: CUBE,
        points_mm: vec![Vec3::new(2.0, 5.0, 5.0)],
        diameter_mm: 2.0,
    };

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("a line needs two points");

    assert!(matches!(error, ApplyError::Failed { object, .. } if object == CUBE));
}
