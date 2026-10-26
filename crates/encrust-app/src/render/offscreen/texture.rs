use super::SIZE;
use super::marks::Marks;
use super::paint::{ball, gpu, paint, read_back};
use crate::camera::OrbitCamera;
use crate::render::gpu::{FrameInput, ModelDraw, ReliefDraw, ViewportResources};
use crate::render::vertex::{ModelInstance, NOT_MARKED};
use crate::scene::Mapped;
use crate::ui::theme;
use core_geometry::{Heightmap, Transform, UvMap, Vec2, Vec3};
use std::sync::Arc;

/// The texture the Relief tool shows has to reach the surface: a model drawn with a map
/// that is white over half its area cannot look like the same model drawn flat.
#[test]
fn a_texture_shows_on_the_model_it_would_be_pressed_into() {
    let mesh = Arc::new(core_geometry::transform_mesh(
        &ball(30.0, 32, 48),
        Transform::from_translation(Vec3::new(75.0, 40.0, 40.0)),
    ));
    let instance = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let camera = OrbitCamera {
        target: Vec3::new(75.0, 40.0, 40.0),
        distance_mm: 150.0,
        ..OrbitCamera::default()
    };

    let Some(flat) = paint(
        camera,
        None,
        &[ModelDraw::whole(Arc::clone(&mesh), instance)],
        &[],
        Marks::default(),
    ) else {
        return;
    };

    // White over the right half of the image, black over the left, planar over X.
    let heights = Heightmap::new(2, 1, vec![0.0, 1.0]).expect("two samples over two pixels");
    let bounds = mesh.aabb().expect("the ball has faces");
    let width = bounds.maxs.x - bounds.mins.x;
    let uvs = UvMap::whole(
        mesh.faces
            .iter()
            .map(|face| {
                face.map(|index| {
                    let vertex = mesh.vertices[index as usize];
                    Vec2::new((vertex.x - bounds.mins.x) / width, 0.5)
                })
            })
            .collect(),
    );
    let textured = paint_textured(
        camera,
        &ReliefDraw {
            mesh,
            mapped: Arc::new(Mapped {
                names: vec!["half.png".to_owned()],
                uvs,
                heights: vec![heights],
            }),
            instance,
        },
    )
    .expect("the second frame draws too");

    let changed = flat
        .as_chunks::<4>()
        .0
        .iter()
        .zip(textured.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count();
    assert!(
        changed > SIZE as usize,
        "only {changed} pixel(s) differ, so the texture never reached the surface"
    );
}

/// One frame of a model drawn with its texture on it.
pub(super) fn paint_textured(camera: OrbitCamera, draw: &ReliefDraw) -> Option<Vec<u8>> {
    let (device, queue) = gpu()?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut resources = ViewportResources::new(device, format);
    resources.prepare(
        device,
        queue,
        FrameInput {
            reliefs: std::slice::from_ref(draw),
            view_projection: camera.view_projection(1.0),
            eye: camera.eye(),
            section_mm: None,
            lines: &[],
            wide_lines: &[],
            models: &[],
            markers: &[],
            solids: &[],
            cap: &[],
            bottom_cut_mm: None,
            bottom_cap: &[],
            label: &[],
            atlas: None,
            cuts: &[],
            cut_surfaces: &[],
            pockets: &[],
            bands: &[],
            band_floor_mm: 0.0,
            volume_mm: None,
            cut_line: None,
            xray: false,
            floor: None,
            grid: false,
            light: false,
        },
    );
    Some(read_back(device, queue, format, &mut resources))
}
