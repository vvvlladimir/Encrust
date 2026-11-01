//! The key light's shadow in the render view: soft at rest, a single tap while the camera moves.

use std::sync::Arc;

use core_geometry::{Transform, Vec3, cuboid};

use super::marks::Marks;
use super::paint::{draw_frame, frame_input, gpu, read_back};
use crate::camera::OrbitCamera;
use crate::render::gpu::{Desk, Look, ModelDraw, ViewportResources};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// Neighbouring pixels across a row that differ by more than `by` in summed channels.
fn steps_over(pixels: &[u8], by: u32) -> usize {
    let sum = |pixel: &[u8; 4]| pixel[..3].iter().map(|c| u32::from(*c)).sum::<u32>();
    let (pixels, _) = pixels.as_chunks::<4>();
    pixels
        .chunks(super::SIZE as usize)
        .flat_map(|row| row.windows(2))
        .filter(|pair| sum(&pair[0]).abs_diff(sum(&pair[1])) > by)
        .count()
}

#[test]
fn the_key_shadow_is_hard_while_the_camera_moves_and_soft_at_rest() {
    let Some((device, queue)) = gpu() else {
        return;
    };
    let post = cuboid(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 80.0));
    let instance = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let models = [ModelDraw::whole(Arc::new(post), instance)];
    let look = Look {
        pivot_mm: 250.0,
        desk: Desk {
            rect: [-150.0, -100.0, 150.0, 100.0],
            mat_mm: [300.0, 200.0],
        },
        ..Look::default()
    };
    let camera = OrbitCamera {
        target: Vec3::new(0.0, 0.0, 20.0),
        distance_mm: 250.0,
        pitch_rad: 0.9,
        ..OrbitCamera::default()
    };
    let marks = |moving| Marks {
        look: Some(look),
        moving,
        ..Marks::default()
    };
    // A frame at rest first, which bakes the key's map that a moving one reads as it stands.
    let frame = |moving| {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut resources = ViewportResources::new(device, format);
        draw_frame(&mut resources, &camera, &models, marks(false));
        let input = frame_input(&camera, (None, &[]), (&models, &[]), marks(moving));
        resources.prepare(device, queue, input);
        read_back(device, queue, format, &mut resources)
    };
    let (resting, moving) = (steps_over(&frame(false), 24), steps_over(&frame(true), 24));
    assert!(
        moving > 2 * resting,
        "a hard shadow's edge steps sharply, {moving} steps moving against {resting} at rest"
    );
}
