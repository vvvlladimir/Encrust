//! The render view's own planes, held only while it is on.

use std::sync::Arc;

use core_geometry::{Transform, Vec3};

use super::marks::Marks;
use super::paint::{ball, draw_frame, gpu};
use crate::camera::OrbitCamera;
use crate::render::gpu::{Look, ModelDraw, ViewportResources};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::ui::theme;

/// Draws one frame of `models`, in the render view under `look`.
fn draw(resources: &mut ViewportResources, models: &[ModelDraw], look: Option<Look>) {
    let camera = OrbitCamera {
        distance_mm: 170.0,
        ..OrbitCamera::default()
    };
    let marks = Marks {
        look,
        ..Marks::default()
    };
    draw_frame(resources, &camera, models, marks);
}

#[test]
fn leaving_the_render_view_lets_its_planes_go() {
    let Some((device, _)) = gpu() else {
        return;
    };
    let mut resources = ViewportResources::new(device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let instance = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let models = [ModelDraw::whole(
        Arc::new(core_geometry::transform_mesh(
            &ball(30.0, 24, 32),
            Transform::from_translation(Vec3::new(0.0, 0.0, 30.0)),
        )),
        instance,
    )];
    let studio = Look {
        pivot_mm: 170.0,
        ..Look::default()
    };
    draw(&mut resources, &models, Some(studio));
    assert!(
        resources.holds_render_planes(),
        "the render view at rest draws into them"
    );
    draw(&mut resources, &models, None);
    assert!(
        !resources.holds_render_planes(),
        "the working view holds none of them"
    );
}
