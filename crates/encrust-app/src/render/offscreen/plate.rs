use super::marks::{Marks, paint_ball, paint_marked_ball};
use super::paint::{ball, paint};
use crate::camera::OrbitCamera;
use crate::render::gpu::{CutLine, ExposureBand, Floor, ModelDraw};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;
use core_geometry::{Mesh, Transform, Vec2, Vec3};
use std::sync::Arc;

/// A band tints the surface it covers and leaves the rest of the model alone, and shows
/// nothing at all below the floor, because there the bottom block keeps the resin's own
/// exposure whatever the band says. See ADR 0090.
#[test]
fn an_exposure_band_tints_only_the_height_it_covers() {
    let band = ExposureBand::new(40.0, 60.0, theme::band_tint(0));
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let banded = paint_ball(&[band], 0.0, None).expect("the second frame draws too");
    let floored = paint_ball(&[band], 100.0, None).expect("the third frame draws too");

    let changed = |a: &Vec<u8>, b: &Vec<u8>| {
        a.as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .filter(|(before, after)| before != after)
            .count()
    };
    assert!(
        changed(&plain, &banded) > 0,
        "a band has to show on the model at all"
    );
    assert_eq!(
        changed(&plain, &floored),
        0,
        "a band below the floor is not printed, so it may not be drawn either"
    );
}

/// A model standing past the build volume is marked there and nowhere else: the ball
/// reaches x = 115 mm, so a volume 100 mm wide cuts through it and one 200 mm wide does not.
#[test]
fn a_model_past_the_build_volume_is_marked_there() {
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let roomy = paint_ball(&[], 0.0, Some(Vec3::splat(200.0))).expect("the second frame");
    let tight = paint_ball(&[], 0.0, Some(Vec3::new(100.0, 200.0, 200.0))).expect("the third");

    assert_eq!(plain, roomy, "a model inside the volume is drawn as it was");
    assert_ne!(plain, tight, "the part past x = 100 mm has to show");
}

/// The Cut tool's plane is traced where it crosses the model it is set on, and nowhere on
/// a model whose box it does not reach.
#[test]
fn the_cut_plane_is_traced_across_the_model_it_is_set_on() {
    let Some(plain) = paint_ball(&[], 0.0, None) else {
        return;
    };
    let line = |bounds| CutLine {
        normal: Vec3::Z,
        offset_mm: 45.0,
        bounds,
    };
    // The ball of radius 40 stands at z = 45, so the plane runs round its equator.
    let ball = core_geometry::Aabb::new(Vec3::new(35.0, 0.0, 5.0), Vec3::new(115.0, 80.0, 85.0));
    let elsewhere = core_geometry::Aabb::new(Vec3::splat(-50.0), Vec3::splat(-10.0));
    let traced = paint_marked_ball(Marks {
        cut_line: Some(line(ball)),
        ..Marks::default()
    })
    .expect("the second frame draws too");
    let missed = paint_marked_ball(Marks {
        cut_line: Some(line(elsewhere)),
        ..Marks::default()
    })
    .expect("the third frame draws too");

    // Compared with the untraced frame rather than with the token: this target is sRGB and
    // lightens what is written to it, so no pixel holds the token as it is.
    let brightness = |pixel: &[u8; 4]| {
        pixel[..3]
            .iter()
            .map(|channel| u32::from(*channel))
            .sum::<u32>()
    };
    let darkened = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(traced.as_chunks::<4>().0)
        .filter(|(before, after)| brightness(after) + 60 < brightness(before))
        .count();
    assert!(
        darkened > 500,
        "a ball 80 mm across has a long equator to darken, got {darkened} pixels"
    );
    assert_eq!(
        plain, missed,
        "a plane set on another model traces nothing on this one"
    );
}

/// A model standing a little over the plate darkens the plate under it, and only darkens:
/// the shadow is black laid over what is there. Only what the frame without a floor
/// already paints is compared, since the floor also fills the plate where nothing stands. Lifted well clear of the plate, the same
/// model casts nothing, because the shadow is a contact shadow and not the key light's.
#[test]
fn a_model_near_the_plate_darkens_it_and_one_high_above_does_not() {
    let floor = Floor {
        near_corner_mm: Vec2::ZERO,
        size_mm: Vec2::new(150.0, 80.0),
    };
    let Some(bare) = paint_over_a_lit_plate(10.0, None) else {
        return;
    };
    let shaded = paint_over_a_lit_plate(10.0, Some(floor)).expect("the second frame draws");
    let changed: Vec<_> = bare
        .as_chunks::<4>()
        .0
        .iter()
        .zip(shaded.as_chunks::<4>().0)
        .filter(|(before, after)| before[..3] != [0, 0, 0] && before != after)
        .collect();
    assert!(
        !changed.is_empty(),
        "the shadow has to reach the plate at all"
    );
    assert!(
        changed
            .iter()
            .all(|(before, after)| (0..3).all(|channel| after[channel] <= before[channel])),
        "a shadow only takes light away"
    );

    let high_bare = paint_over_a_lit_plate(80.0, None).expect("the third frame draws");
    let high_shaded = paint_over_a_lit_plate(80.0, Some(floor)).expect("the fourth frame draws");
    let darkened = high_bare
        .as_chunks::<4>()
        .0
        .iter()
        .zip(high_shaded.as_chunks::<4>().0)
        .filter(|(before, after)| before[..3] != [0, 0, 0] && before != after)
        .count();
    assert_eq!(
        darkened, 0,
        "a model past the shadow's reach casts nothing on the plate"
    );
}

/// A slab standing on the plate and cut just over it: the model's floor lies in the plate,
/// and the shadow laid on the plate must not show through the cap over it.
#[test]
fn the_shadow_on_the_plate_never_shows_through_a_cap() {
    let floor = Floor {
        near_corner_mm: Vec2::ZERO,
        size_mm: Vec2::new(150.0, 80.0),
    };
    let slab = slab(Vec3::new(35.0, 20.0, 0.0), Vec3::new(115.0, 60.0, 16.0));
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cap_colour =
        ModelInstance::new(Transform::default(), theme::scene().section_cap, NOT_MARKED);
    let models = [ModelDraw::whole(Arc::clone(&slab), object)];
    let solids = [ModelDraw::whole(Arc::clone(&slab), cap_colour)];
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 0.0),
        distance_mm: 300.0,
        pitch_rad: 1.0,
        ..OrbitCamera::default()
    };
    let frame = |floor| {
        let marks = Marks {
            floor,
            ..Marks::default()
        };
        paint(camera, Some(0.8), &models, &solids, marks)
    };
    let Some(bare) = frame(None) else {
        return;
    };
    let shaded = frame(Some(floor)).expect("the second frame draws too");

    let darkened = bare
        .as_chunks::<4>()
        .0
        .iter()
        .zip(shaded.as_chunks::<4>().0)
        .filter(|(bare, shaded)| bare[..3] != [0, 0, 0] && bare != shaded)
        .count();
    assert_eq!(darkened, 0, "{darkened} pixel(s) of the cap darkened");
}

/// A closed box between two corners, wound outward.
pub(super) fn slab(low: Vec3, high: Vec3) -> Arc<Mesh> {
    Arc::new(core_geometry::cuboid(low, high))
}

/// Two meshes lying in one plane, the second pulled towards the eye: it wins every pixel
/// of it, where unpulled the two would take the plane by turns. A pad on a raft, or the
/// pads of two support groups standing in each other.
#[test]
fn a_pulled_mesh_wins_the_plane_it_shares_with_another() {
    let square = |faces: Vec<[u32; 3]>| {
        Arc::new(Mesh::new(
            vec![
                Vec3::new(55.0, 20.0, 0.0),
                Vec3::new(95.0, 20.0, 0.0),
                Vec3::new(95.0, 60.0, 0.0),
                Vec3::new(55.0, 20.0, 0.0) + Vec3::new(0.0, 40.0, 0.0),
            ],
            faces,
        ))
    };
    let under = ModelDraw::whole(
        square(vec![[0, 1, 2], [0, 2, 3]]),
        ModelInstance::new(Transform::default(), theme::support_tint(1), NOT_MARKED),
    );
    let over = || {
        ModelDraw::whole(
            square(vec![[0, 1, 3], [1, 2, 3]]),
            ModelInstance::new(Transform::default(), theme::support_tint(0), NOT_MARKED)
                .pulled(1e-4),
        )
    };
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 0.0),
        distance_mm: 200.0,
        ..OrbitCamera::default()
    };
    let Some(alone) = paint(camera, None, &[over()], &[], Marks::default()) else {
        return;
    };
    let shared = paint(camera, None, &[under, over()], &[], Marks::default())
        .expect("the second frame draws too");
    let lost = alone
        .as_chunks::<4>()
        .0
        .iter()
        .zip(shared.as_chunks::<4>().0)
        .filter(|(alone, shared)| alone != shared)
        .count();
    assert_eq!(
        lost, 0,
        "{lost} pixel(s) of the plane went to the mesh under it"
    );
}

/// A disc 40 mm across hanging `height_mm` over a wide lit disc lying just under the
/// plate, which stands in for the plate the shadow is laid on; seen from the front, so
/// the plate under the hanging disc shows past its front edge.
pub(super) fn paint_over_a_lit_plate(height_mm: f32, floor: Option<Floor>) -> Option<Vec<u8>> {
    let disc = Arc::new(ball(40.0, 16, 24));
    let placed = |translation, scale| {
        ModelDraw::whole(
            Arc::clone(&disc),
            ModelInstance::new(
                Transform {
                    translation,
                    scale,
                    ..Transform::default()
                },
                theme::scene().object,
                NOT_MARKED,
            ),
        )
    };
    let models = [
        placed(Vec3::new(75.0, 40.0, -1.0), Vec3::new(2.5, 1.5, 0.01)),
        placed(Vec3::new(75.0, 40.0, height_mm), Vec3::new(0.5, 0.5, 0.02)),
    ];
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 0.0),
        distance_mm: 200.0,
        ..OrbitCamera::default()
    };
    paint(
        camera,
        None,
        &models,
        &[],
        Marks {
            floor,
            ..Marks::default()
        },
    )
}
