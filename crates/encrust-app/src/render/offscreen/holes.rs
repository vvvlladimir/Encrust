use super::SECTION_MM;
use super::marks::{Marks, cut_surfaces};
use super::paint::{ball, differing, paint};
use crate::camera::OrbitCamera;
use crate::render::callback::cuts_of;
use crate::render::gpu::{CutDraw, DrainCut, ModelDraw};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;
use core_geometry::{Bvh, Mesh, Transform, Vec3};
use core_volume::{Cancel, DrainHole, HollowSettings, drill, hollow};
use std::sync::{Arc, OnceLock};

/// The ball every frame is drawn from: the model, its hierarchy and its hollowed shell.
/// Hollowing it is the dearest thing in this file and the answer never changes.
pub(super) fn hollowed_ball() -> &'static (Mesh, Bvh, Arc<Mesh>, std::ops::Range<usize>) {
    pub(super) static BALL: OnceLock<(Mesh, Bvh, Arc<Mesh>, std::ops::Range<usize>)> =
        OnceLock::new();
    BALL.get_or_init(|| {
        let mesh = core_geometry::transform_mesh(
            &ball(40.0, 48, 64),
            Transform::from_translation(Vec3::new(75.0, 40.0, 45.0)),
        );
        let bvh = Bvh::build(&mesh);
        let hollowed = hollow(
            &mesh,
            &bvh,
            &HollowSettings {
                thickness_mm: 2.0,
                precision: 0.6,
                ..HollowSettings::default()
            },
            Cancel::never(),
        )
        .expect("the ball hollows");
        (mesh, bvh, Arc::new(hollowed.mesh), hollowed.cavity)
    })
}

/// A hollow ball, with the holes of [`holes`] in it or without them.
pub(super) fn drilled_ball(drilled: bool) -> (Arc<Mesh>, Arc<Mesh>, Vec<DrainHole>) {
    let (_, _, shell, _) = hollowed_ball();
    let holes = if drilled { holes() } else { Vec::new() };
    let bodies = drill(&holes, &[]).expect("the holes drill");
    (Arc::clone(shell), Arc::new(bodies), holes)
}

/// Both mouths stand eight millimetres clear of the surface, the way `lift_for` puts them
/// where the surface rises around a hole: one hole through the top, one on the equator, whose
/// lifted tube stands in the air across the cutting plane.
pub(super) fn holes() -> Vec<DrainHole> {
    vec![
        DrainHole {
            at: Vec3::new(75.0, 40.0, 85.0),
            axis: Vec3::NEG_Z,
            diameter_mm: 4.0,
            depth_mm: 4.0,
            taper: 1.0,
            lift_mm: 8.0,
            shape: core_volume::HoleShape::Round,
            turn_deg: 0.0,
            kind: core_volume::HoleKind::Drain,
        },
        DrainHole {
            at: Vec3::new(114.7, 40.0, 50.0),
            axis: Vec3::new(-39.7, 0.0, -5.0),
            diameter_mm: 4.0,
            depth_mm: 4.0,
            taper: 1.0,
            lift_mm: 8.0,
            shape: core_volume::HoleShape::Round,
            turn_deg: 0.0,
            kind: core_volume::HoleKind::Drain,
        },
    ]
}

/// A hole takes material away and can repaint what shows through itself. What it can never
/// do is put something on screen where the model was not, and a cap painted over a tube
/// standing in the air does exactly that.
#[test]
fn drilling_a_model_paints_nothing_where_the_model_was_not() {
    let Some(solid) = render(false) else {
        return;
    };
    let drilled = render(true).expect("the second frame draws too");

    let background = |pixel: &[u8; 4]| pixel[..3] == [0, 0, 0];
    let painted = solid
        .as_chunks::<4>()
        .0
        .iter()
        .zip(drilled.as_chunks::<4>().0)
        .filter(|(solid, drilled)| background(solid) && !background(drilled))
        .count();
    assert_eq!(
        painted, 0,
        "{painted} pixel(s) stand in the air around a hole"
    );
}

/// A hole is drawn from the cut's own body, so what shows inside it is the far wall of
/// that body rather than whatever of the model stands behind the opening. See ADR 0201.
#[test]
fn a_hole_shows_the_wall_of_the_cut_that_made_it() {
    // Head-on at the hole on the equator, close enough that its four millimetres cover a
    // good part of the frame.
    let camera = OrbitCamera {
        target: Vec3::new(110.0, 40.0, 50.0),
        distance_mm: 22.0,
        yaw_rad: std::f32::consts::PI,
        pitch_rad: 0.1,
        ..OrbitCamera::default()
    };
    let frame = |walled| {
        let (mesh, bodies, holes) = drilled_ball(true);
        let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
        let models = [ModelDraw::whole(Arc::clone(&mesh), object)];
        let surfaces = match walled {
            true => cut_surfaces(&mesh, &bodies, object),
            false => Vec::new(),
        };
        let cuts = cuts_of(&holes, &[], Transform::default());
        paint(
            camera,
            None,
            &models,
            &[],
            Marks {
                cuts: &cuts,
                cut_surfaces: &surfaces,
                ..Marks::default()
            },
        )
    };

    let Some(bare) = frame(false) else {
        return;
    };
    let walled = frame(true).expect("the second frame draws too");

    let background = |pixel: &[u8; 4]| pixel[..3] == [0, 0, 0];
    let (changed, painted) = bare
        .as_chunks::<4>()
        .0
        .iter()
        .zip(walled.as_chunks::<4>().0)
        .fold((0, 0), |(changed, painted), (bare, walled)| {
            (
                changed + usize::from(bare != walled),
                painted + usize::from(background(bare) && !background(walled)),
            )
        });
    assert!(
        changed > 2_000,
        "a 4 mm hole seen from 22 mm away is wide on screen: {changed} pixels"
    );
    assert_eq!(
        painted, 0,
        "{painted} pixel(s) of a wall stand where the model was not"
    );
}

/// A polygonal hole takes away its own outline and not the circle round it: a square with its
/// corners on the diameter leaves the model standing between its sides and that circle.
#[test]
fn a_square_hole_takes_away_less_of_the_model_than_its_circle() {
    let camera = OrbitCamera {
        target: Vec3::new(110.0, 40.0, 50.0),
        distance_mm: 22.0,
        yaw_rad: std::f32::consts::PI,
        pitch_rad: 0.1,
        ..OrbitCamera::default()
    };
    let (mesh, _, _) = drilled_ball(false);
    let square: Vec<DrainHole> = holes()
        .into_iter()
        .map(|hole| DrainHole {
            shape: core_volume::HoleShape::Polygon { sides: 4 },
            turn_deg: 45.0,
            ..hole
        })
        .collect();
    let bodies = Arc::new(drill(&square, &[]).expect("the square holes drill"));
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let models = [ModelDraw::whole(Arc::clone(&mesh), object)];
    let surfaces = cut_surfaces(&mesh, &bodies, object);
    let frame = |cuts: &[DrainCut], surfaces: &[CutDraw]| {
        paint(
            camera,
            None,
            &models,
            &[],
            Marks {
                cuts,
                cut_surfaces: surfaces,
                ..Marks::default()
            },
        )
    };

    let Some(whole) = frame(&[], &[]) else {
        return;
    };
    let squared = cuts_of(&square, &[], Transform::default());
    let mut rounded = squared.clone();
    for cut in &mut rounded {
        cut.corner = [0.0; 4];
    }
    let changed = |cuts: &[DrainCut]| {
        let drilled = frame(cuts, &surfaces).expect("a later frame draws too");
        whole
            .as_chunks::<4>()
            .0
            .iter()
            .zip(drilled.as_chunks::<4>().0)
            .filter(|(whole, drilled)| whole != drilled)
            .count()
    };
    let (by_square, by_circle) = (changed(&squared), changed(&rounded));
    // A square holds 2/pi of the circle its corners are on.
    assert!(
        (by_square as f32) < 0.8 * by_circle as f32,
        "the square opens {by_square} pixels, its circle {by_circle}"
    );
}

/// A hole the section runs through still shows its wall under the plane: what stood in front
/// of that wall is cut away, but the cap stands in its place.
#[test]
fn a_hole_the_section_runs_through_shows_its_wall_under_the_cap() {
    // Down onto the hole on the equator, which the plane cuts along its length.
    let camera = OrbitCamera {
        target: Vec3::new(112.0, 40.0, 50.0),
        distance_mm: 30.0,
        yaw_rad: 0.0,
        pitch_rad: 1.2,
        ..OrbitCamera::default()
    };
    let Some(bare) = sectioned_ball(camera, false, true) else {
        return;
    };
    let walled = sectioned_ball(camera, true, true).expect("the second frame draws too");

    let changed = differing(&bare, &walled);
    assert!(
        changed > 4_000,
        "the wall under the cap is drawn, not only where shell under the plane stands in front of it: {changed} pixels"
    );
}

/// The top cap is counted along the stretch of a view ray above the plane, which is in front
/// of the plane only for an eye above it. From under the plane, through a hole, it is not.
#[test]
fn the_cap_is_never_seen_from_the_side_of_the_plane_it_does_not_face() {
    // From under the plane, up into the mouth of the hole on the equator.
    let camera = OrbitCamera {
        target: Vec3::new(115.0, 40.0, 50.0),
        distance_mm: 30.0,
        yaw_rad: 0.0,
        pitch_rad: -0.4,
        ..OrbitCamera::default()
    };
    let Some(uncapped) = sectioned_ball(camera, true, false) else {
        return;
    };
    let capped = sectioned_ball(camera, true, true).expect("the second frame draws too");

    let changed = differing(&uncapped, &capped);
    assert_eq!(changed, 0, "{changed} pixel(s) of cap seen from under it");
}

/// The drilled ball cut at [`SECTION_MM`], with the inside of its holes drawn or not, and
/// counted into the cap or not.
pub(super) fn sectioned_ball(camera: OrbitCamera, walled: bool, counted: bool) -> Option<Vec<u8>> {
    let (mesh, bodies, holes) = drilled_ball(true);
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cap_colour =
        ModelInstance::new(Transform::default(), theme::scene().section_cap, NOT_MARKED);
    let models = [ModelDraw::whole(Arc::clone(&mesh), object)];
    let solids = match counted {
        true => vec![
            ModelDraw::whole(Arc::clone(&mesh), cap_colour),
            ModelDraw::whole(Arc::clone(&bodies), cap_colour),
        ],
        false => Vec::new(),
    };
    let surfaces = match walled {
        true => cut_surfaces(&mesh, &bodies, object),
        false => Vec::new(),
    };
    let cuts = cuts_of(&holes, &[], Transform::default());
    let marks = Marks {
        cuts: &cuts,
        cut_surfaces: &surfaces,
        ..Marks::default()
    };
    paint(camera, Some(SECTION_MM), &models, &solids, marks)
}

/// The frame as the window would draw it, or `None` on a machine with no adapter to draw
/// it with.
pub(super) fn render(drilled: bool) -> Option<Vec<u8>> {
    painted_ball(drilled, true)
}

/// The same frame, with the inside of the cuts drawn or left out of it.
pub(super) fn painted_ball(drilled: bool, walled: bool) -> Option<Vec<u8>> {
    let (mesh, bodies, drilled) = drilled_ball(drilled);
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cap_colour =
        ModelInstance::new(Transform::default(), theme::scene().section_cap, NOT_MARKED);
    let models = vec![ModelDraw::whole(Arc::clone(&mesh), object)];
    // The window counts the cut bodies into the cap without drawing them; see ADR 0075.
    let solids = vec![
        ModelDraw::whole(Arc::clone(&mesh), cap_colour),
        ModelDraw::whole(Arc::clone(&bodies), cap_colour),
    ];
    let cuts: Vec<DrainCut> = cuts_of(&drilled, &[], Transform::default());
    let cut_surfaces = match walled {
        true => cut_surfaces(&mesh, &bodies, object),
        false => Vec::new(),
    };
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    let marks = Marks {
        cuts: &cuts,
        cut_surfaces: &cut_surfaces,
        ..Marks::default()
    };
    paint(camera, Some(SECTION_MM), &models, &solids, marks)
}
