//! The render view: layers stepped into a model, a clear resin, creases shaded at rest.

use std::sync::Arc;

use core_geometry::{Transform, Vec2, Vec3, cuboid};

use super::marks::{Marks, paint_marked_ball};
use super::paint::paint;
use crate::camera::OrbitCamera;
use crate::render::gpu::{Floor, Look, ModelDraw};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// Far coarser than any printer cuts, so the steps span many pixels of the 80 mm ball.
const COARSE_LAYER_MM: f32 = 2.0;

/// Pixels that differ between two frames by more than `by` in summed channels, and those
/// of them where `after` is the darker.
fn darker(before: &[u8], after: &[u8], by: u32) -> (usize, usize) {
    let sum = |pixel: &[u8; 4]| pixel[..3].iter().map(|c| u32::from(*c)).sum::<u32>();
    let pairs = || {
        before
            .as_chunks::<4>()
            .0
            .iter()
            .zip(after.as_chunks::<4>().0)
    };
    let changed = pairs()
        .filter(|(a, b)| sum(a).abs_diff(sum(b)) > by)
        .count();
    let darkened = pairs().filter(|(a, b)| sum(b) + by < sum(a)).count();
    (changed, darkened)
}

fn studio() -> Look {
    Look {
        pivot_mm: 170.0,
        ..Look::default()
    }
}

#[test]
fn layers_step_into_the_model_only_while_asked_for() {
    let Some(plain) = paint_marked_ball(Marks {
        look: Some(studio()),
        ..Marks::default()
    }) else {
        return;
    };
    let layered = paint_marked_ball(Marks {
        look: Some(Look {
            layer_height_mm: COARSE_LAYER_MM,
            ..studio()
        }),
        ..Marks::default()
    })
    .expect("the adapter drew once");
    let (changed, _) = darker(&plain, &layered, 0);
    assert!(
        changed > 5_000,
        "{COARSE_LAYER_MM} mm layers over a ball 80 mm across shade thousands of pixels, got {changed}"
    );
}

#[test]
fn a_clear_resin_shows_the_plate_through_a_hollow_model() {
    let floor = Some(Floor {
        near_corner_mm: Vec2::ZERO,
        size_mm: Vec2::new(150.0, 80.0),
    });
    let frame = |attenuation_mm| {
        paint_marked_ball(Marks {
            look: Some(Look {
                attenuation_mm,
                ..studio()
            }),
            floor,
            presented: true,
            ..Marks::default()
        })
    };
    let Some(opaque) = frame(0.0) else {
        return;
    };
    let clear = frame(40.0).expect("the adapter drew once");
    let (changed, _) = darker(&opaque, &clear, 30);
    assert!(
        changed > 20_000,
        "a hollow ball 80 mm across lets the plate through much of itself, got {changed} pixels"
    );
}

/// A wall standing on a slab: the crease along its foot is the only enclosed place.
fn wall_on_a_slab() -> Vec<ModelDraw> {
    let colour = theme::scene().object;
    let block = |low: Vec3, high: Vec3| {
        ModelDraw::whole(
            Arc::new(cuboid(low, high)),
            ModelInstance::new(Transform::default(), colour, NOT_MARKED),
        )
    };
    vec![
        block(Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 40.0, 4.0)),
        block(Vec3::new(18.0, 0.0, 4.0), Vec3::new(22.0, 40.0, 30.0)),
    ]
}

#[test]
fn creases_are_shaded_in_the_copy_and_nothing_else_is_lightened() {
    let camera = OrbitCamera {
        target: Vec3::new(20.0, 20.0, 6.0),
        distance_mm: 60.0,
        ..OrbitCamera::default()
    };
    let models = wall_on_a_slab();
    let look = Some(Look {
        projection: camera.projection(1.0),
        ..studio()
    });
    let frame = |presented| {
        paint(
            camera,
            None,
            &models,
            &[],
            Marks {
                look,
                presented,
                ..Marks::default()
            },
        )
    };
    let Some(drawn) = frame(false) else {
        return;
    };
    let copied = frame(true).expect("the adapter drew once");
    let (changed, darkened) = darker(&drawn, &copied, 6);
    assert!(
        darkened > 300,
        "the wall's foot is shaded, got {darkened} pixels darker"
    );
    assert_eq!(changed, darkened, "the shading only ever takes light away");
}

#[test]
fn layers_too_fine_to_see_still_frost_a_clear_part() {
    let floor = Some(Floor {
        near_corner_mm: Vec2::ZERO,
        size_mm: Vec2::new(150.0, 80.0),
    });
    let frame = |layer_height_mm| {
        paint_marked_ball(Marks {
            look: Some(Look {
                layer_height_mm,
                attenuation_mm: 300.0,
                ..studio()
            }),
            floor,
            presented: true,
            ..Marks::default()
        })
    };
    let Some(smooth) = frame(0.0) else {
        return;
    };
    let printed = frame(0.05).expect("the adapter drew once");
    let (changed, _) = darker(&smooth, &printed, 12);
    assert!(
        changed > 10_000,
        "0.05 mm layers frost a ball 80 mm across, got {changed} pixels changed"
    );
}

#[test]
fn a_clear_resin_is_drawn_opaque_while_the_camera_moves() {
    let frame = |attenuation_mm| {
        paint_marked_ball(Marks {
            look: Some(Look {
                attenuation_mm,
                ..studio()
            }),
            presented: true,
            moving: true,
            ..Marks::default()
        })
    };
    let Some(opaque) = frame(0.0) else {
        return;
    };
    let clear = frame(40.0).expect("the adapter drew once");
    assert!(opaque == clear, "a moving frame draws no clear pass");
}

#[test]
fn a_thin_wall_glows_with_the_key_behind_it_once_the_camera_rests() {
    let mut camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    camera.orbit(3.4, 0.1);
    let (shell, _, _) = super::holes::drilled_ball(false);
    let models = vec![ModelDraw::whole(
        shell,
        ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED),
    )];
    let frame = |moving| {
        paint(
            camera,
            None,
            &models,
            &[],
            Marks {
                look: Some(studio()),
                moving,
                ..Marks::default()
            },
        )
    };
    let Some(moving) = frame(true) else {
        return;
    };
    let resting = frame(false).expect("the adapter drew once");
    // At rest the key's shadow is baked too, which only darkens.
    let (changed, darkened) = darker(&moving, &resting, 3);
    let lightened = changed - darkened;
    assert!(
        lightened > 1_000,
        "a shell glows lighter at rest, got {lightened} pixels lighter"
    );
}
