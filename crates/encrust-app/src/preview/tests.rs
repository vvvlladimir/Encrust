use std::path::Path;

use super::picture::MAX_PREVIEW_PX;
use super::*;
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
use core_raster::{Grey, LayerMask};
use core_raster::{PixelPitch, Shading};

use crate::scene::ImportSummary;
use crate::status::Status;

/// Panel of 64 x 32 pixels at a 0.2 mm pitch: 12.8 x 6.4 mm of build area.
fn settings() -> RasterSettings {
    RasterSettings {
        width_px: 64,
        height_px: 32,
        pitch: PixelPitch { x: 0.2, y: 0.2 },
        mirror_x: false,
        mirror_y: false,
        shading: Shading::Coverage,
        grey: Grey::default(),
        blur_px: 0,
    }
}

/// A preview of a 4 x 4 mm box `count` half-millimetre layers tall, with its first
/// window already cut.
fn preview_with_stack(count: usize) -> Preview {
    let mesh = Arc::new(box_mesh(count as Scalar * 0.5));
    let windows = Windows::new(
        &mesh,
        core_slicer::SliceSettings {
            layer_height: 0.5,
            ..core_slicer::SliceSettings::default()
        },
        core_slicer::WINDOW_LAYERS,
    )
    .expect("a box slices");
    let mut preview = Preview {
        source: Some(Stack {
            solid: Arc::new(Solid::from(mesh.as_ref().clone())),
            windows,
            fingerprint: 7,
            cut: None,
        }),
        ..Preview::default()
    };
    preview.cut_window().expect("a box slices");
    preview
}

/// A 4 x 4 mm box `height` millimetres tall, standing on the plate.
fn box_mesh(height: Scalar) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::new(4.0, 4.0, height))
}

#[test]
fn a_box_starting_in_mid_air_is_an_island_on_its_first_layer_only() {
    // One box on the plate, and one beside it from 1 mm up: layer 2 is its first.
    let mut mesh = box_mesh(2.0);
    let floating = box_mesh(1.0);
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(
        floating
            .vertices
            .iter()
            .map(|v| Vec3::new(v.x + 6.0, v.y, v.z + 1.0)),
    );
    mesh.faces.extend(
        floating
            .faces
            .iter()
            .map(|face| face.map(|index| index + base)),
    );
    let windows = Windows::new(
        &mesh,
        core_slicer::SliceSettings {
            layer_height: 0.5,
            ..core_slicer::SliceSettings::default()
        },
        core_slicer::WINDOW_LAYERS,
    )
    .expect("two boxes slice");
    let mut preview = Preview {
        source: Some(Stack {
            solid: Arc::new(Solid::from(mesh)),
            windows,
            fingerprint: 7,
            cut: None,
        }),
        ..Preview::default()
    };

    let mut island_pixels = |layer: usize| {
        preview.set_layer(layer);
        let shown = preview.full(&settings()).expect("the stack has layers");
        shown
            .islands
            .to_mask()
            .pixels()
            .iter()
            .filter(|p| **p > 0)
            .count()
    };
    assert_eq!(island_pixels(0), 0, "the plate holds the first layer");
    assert_eq!(island_pixels(1), 0, "the floating box has not started");
    // 4 x 4 mm at 0.2 mm a pixel.
    assert_eq!(
        island_pixels(2),
        400,
        "the floating box starts over nothing"
    );
    assert_eq!(island_pixels(3), 0, "it then stands on itself");
}

fn tetrahedron() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    )
}

fn scene_with_a_model() -> Scene {
    let mesh = tetrahedron();
    let mut scene = Scene::default();
    scene.insert(crate::scene::Imported::new(
        "tetra".to_owned(),
        Arc::new(mesh.clone()),
        Transform::default(),
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&mesh),
        },
    ));
    scene
}

#[test]
fn stepping_inside_one_window_cuts_nothing_again() {
    // Cutting is what a preview must not do on every frame; the window it is showing
    // is kept. See ADR 0068.
    let mut preview = preview_with_stack(4);
    let before = preview.layer_z();
    preview.set_layer(2);
    preview.cut_window().expect("a box slices");

    assert_eq!(preview.layer_count(), 4);
    assert_ne!(preview.layer_z(), before);
    assert!(preview.layer_area_mm2().is_some_and(|area| area > 15.0));
}

#[test]
fn an_empty_preview_has_no_layers_to_show() {
    let mut preview = Preview::default();
    assert_eq!(preview.layer_count(), 0);
    assert_eq!(preview.layer_z(), None);
    assert!(!preview.is_building());
    assert!(preview.mask(&settings()).is_err());
}

#[test]
fn the_slider_never_leaves_the_stack() {
    let mut preview = preview_with_stack(4);
    preview.set_layer(99);
    assert_eq!(preview.layer(), 3, "the last layer of four");
    assert_eq!(
        preview.layer_z(),
        Some(2.0),
        "the top of the fourth half-millimetre layer"
    );
    // Every layer of the stack is the same 4 x 4 mm square.
    let area = preview.layer_area_mm2().expect("the stack has layers");
    assert!((area - 16.0).abs() < 1e-3, "expected 16 mm^2, got {area}");
}

#[test]
fn the_section_is_cut_where_the_mask_is_sampled() {
    let mut preview = preview_with_stack(4);
    preview.set_layer(1);
    assert_eq!(preview.layer_z(), Some(1.0), "the top of the second layer");
    assert_eq!(
        preview.section_z(),
        Some(0.75),
        "the second half-millimetre layer is sampled at its middle"
    );
}

#[test]
fn a_shorter_stack_pulls_the_slider_back() {
    let mut preview = preview_with_stack(8);
    preview.set_layer(7);

    let scene = scene_with_a_model();
    preview
        .build(&scene, Cutting::uniform(0.5))
        .expect("the plate has a model on it");
    let mut status = Status::default();
    while preview.poll(&mut status) {
        std::thread::yield_now();
    }

    // One millimetre of tetrahedron at half a millimetre a layer: two layers.
    assert_eq!(preview.layer_count(), 2);
    assert_eq!(preview.layer(), 1);
    assert_eq!(status, Status::Idle, "a build that worked says nothing");
}

#[test]
fn a_layer_is_rasterised_at_the_panels_aspect_and_shrunk_to_fit() {
    let mut preview = preview_with_stack(2);
    let mask = preview.mask(&settings()).expect("the stack has layers");

    // The panel is smaller than the preview limit, so it is shown pixel for pixel.
    assert_eq!((mask.width(), mask.height()), (64, 32));
    // A 4 x 4 mm square at a 0.2 mm pitch: 20 x 20 pixels.
    assert!(
        (mask.coverage() - 400.0).abs() < 1.0,
        "expected about 400 lit pixels, got {}",
        mask.coverage()
    );
}

#[test]
fn a_height_from_the_prepare_mode_parks_the_slider_on_the_nearest_layer() {
    // Four layers of half a millimetre, topping out at 0.5, 1.0, 1.5 and 2.0.
    let mut preview = preview_with_stack(4);
    preview.show_height(Some(1.3));
    assert_eq!(preview.layer(), 2);
    preview.show_height(Some(-5.0));
    assert_eq!(preview.layer(), 0, "below the stack is its first layer");
    preview.show_height(Some(40.0));
    assert_eq!(preview.layer(), 3, "above it, its last");
    preview.show_height(None);
    assert_eq!(preview.layer(), 3, "an uncut model is the top of the stack");
}

#[test]
fn a_layer_stands_on_the_top_of_the_one_under_it_and_the_first_on_the_plate() {
    // Four layers of half a millimetre, topping out at 0.5, 1.0, 1.5 and 2.0.
    let preview = preview_with_stack(4);
    assert_eq!(
        preview.bottom_of(0),
        None,
        "the first layer stands on the plate"
    );
    assert_eq!(preview.bottom_of(2), Some(1.0));
    assert_eq!(preview.bottom_of(9), None, "past the stack is no layer");
}

#[test]
fn the_layer_shown_goes_back_to_the_prepare_mode_as_its_height() {
    let mut preview = preview_with_stack(4);
    preview.set_layer(1);
    assert_eq!(preview.prepare_height(), Some(Some(1.0)));
    preview.set_layer(3);
    assert_eq!(
        preview.prepare_height(),
        Some(None),
        "the top layer cuts nothing"
    );
    assert_eq!(
        Preview::default().prepare_height(),
        None,
        "no stack, no say"
    );
}

#[test]
fn a_view_zoomed_in_far_enough_is_drawn_from_the_panels_own_pixels() {
    let mut preview = preview_with_stack(2);
    let ctx = egui::Context::default();
    let visible = PixelRect {
        x0: 10,
        y0: 5,
        x1: 30,
        y1: 20,
    };
    let (region, texture) = preview
        .detail(&ctx, &settings(), visible)
        .expect("the stack has layers")
        .expect("twenty pixels fit a texture");
    assert!(region.contains(visible));
    assert_eq!(
        region,
        PixelRect {
            x0: 0,
            y0: 0,
            x1: 40,
            y1: 27
        },
        "grown by half each way, and cut at the panel's edge"
    );
    assert_eq!(texture.size(), [40, 27]);

    let too_much = PixelRect {
        x0: 0,
        y0: 0,
        x1: MAX_PREVIEW_PX + 1,
        y1: 1,
    };
    assert!(
        preview
            .detail(&ctx, &settings(), too_much)
            .expect("no error")
            .is_none()
    );
}

#[test]
fn zooming_keeps_the_pixel_under_the_cursor_where_it_was() {
    let fit = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 320.0));
    let panel = egui::vec2(64.0, 32.0);
    let mut view = MaskView::default();
    let cursor = egui::pos2(100.0, 50.0);
    let before = view.pixel_at(cursor, fit, panel);
    view.zoom_at(4.0, cursor, fit, panel);
    assert!((view.pixel_at(cursor, fit, panel) - before).length() < 1e-3);
    assert_eq!(view.zoom, 4.0);
}

#[test]
fn a_view_cannot_be_zoomed_out_past_the_whole_panel_or_dragged_off_it() {
    let fit = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 320.0));
    let panel = egui::vec2(64.0, 32.0);
    let mut view = MaskView::default();
    view.zoom_at(0.1, egui::pos2(10.0, 10.0), fit, panel);
    assert_eq!(view.zoom, 1.0);
    view.zoom_at(2.0, egui::pos2(320.0, 160.0), fit, panel);
    view.pan(egui::vec2(1.0e6, 1.0e6), fit, panel);
    let corner = view.pixel_at(fit.center(), fit, panel);
    assert_eq!(
        (corner.x, corner.y),
        (0.0, 0.0),
        "the middle stops at the panel's corner"
    );
}

#[test]
fn a_mirrored_panel_leaves_the_preview_the_way_round_the_plate_is() {
    let mut preview = preview_with_stack(2);
    let mask = preview
        .mask(&RasterSettings {
            mirror_x: true,
            ..settings()
        })
        .expect("the stack has layers");

    let half = mask.width() as usize / 2;
    let (left, right) =
        mask.pixels()
            .chunks(mask.width() as usize)
            .fold((0usize, 0usize), |(left, right), row| {
                let lit = |part: &[u8]| part.iter().filter(|&&pixel| pixel > 0).count();
                (left + lit(&row[..half]), right + lit(&row[half..]))
            });
    // The box stands over x = 0..4 mm of a 12.8 mm panel, so it is left of centre.
    assert!(left > 0, "the layer is drawn");
    assert_eq!(right, 0, "the machine's mirroring stays out of the picture");
}

#[test]
fn the_near_edge_of_the_plate_is_drawn_at_the_bottom_of_the_picture() {
    let mut preview = preview_with_stack(2);
    let mask = preview.mask(&settings()).expect("the stack has layers");

    let width = mask.width() as usize;
    let lit = |row: &[u8]| row.iter().any(|&pixel| pixel > 0);
    let rows: Vec<usize> = mask
        .pixels()
        .chunks(width)
        .enumerate()
        .filter(|(_, row)| lit(row))
        .map(|(y, _)| y)
        .collect();

    // The box covers y = 0..4 mm of a 6.4 mm panel, 32 rows at a 0.2 mm pitch: the
    // near edge of the plate, which the viewport draws nearest the viewer.
    assert_eq!(rows.first().copied(), Some(12), "the far end of the box");
    assert_eq!(
        rows.last().copied(),
        Some(31),
        "and the near edge of the plate"
    );
}

#[test]
fn a_stack_cut_from_another_scene_is_stale() {
    let preview = preview_with_stack(2);
    assert!(!preview.is_stale(7));
    assert!(preview.is_stale(8));
    assert!(
        !Preview::default().is_stale(8),
        "nothing sliced yet is not stale, it is empty"
    );
}

#[test]
fn previewing_an_empty_plate_starts_nothing() {
    let mut preview = Preview::default();
    assert!(
        preview
            .build(&Scene::default(), Cutting::uniform(0.5))
            .is_err()
    );
    assert!(!preview.is_building());
}

#[test]
fn moving_or_hiding_a_model_changes_the_fingerprint() {
    let mut scene = scene_with_a_model();
    let placed = stack_fingerprint(&scene, Cutting::uniform(0.05));

    scene.select(None);
    assert_eq!(
        stack_fingerprint(&scene, Cutting::uniform(0.05)),
        placed,
        "selection does not change the stack"
    );

    assert_ne!(
        stack_fingerprint(&scene, Cutting::uniform(0.1)),
        placed,
        "the layer height changes the stack"
    );

    scene.objects_mut()[0].transform.translation = Vec3::new(1.0, 0.0, 0.0);
    let moved = stack_fingerprint(&scene, Cutting::uniform(0.05));
    assert_ne!(moved, placed, "placement changes the stack");

    scene.objects_mut()[0].visible = false;
    assert_ne!(
        stack_fingerprint(&scene, Cutting::uniform(0.05)),
        moved,
        "hiding a model changes the stack"
    );
}

#[test]
fn drilling_a_hole_or_clearing_it_changes_the_fingerprint() {
    let mut scene = scene_with_a_model();
    let solid = stack_fingerprint(&scene, Cutting::uniform(0.05));

    let object = &mut scene.objects_mut()[0];
    let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
    let size = core_volume::HoleSize::round(0.2, 0.2, 1.0);
    let point = mesh.vertices[0];
    let transform = object.transform;
    object
        .hollow
        .add_drain(&mesh, &bvh, point, Vec3::NEG_Z, size, transform);
    let drilled = stack_fingerprint(&scene, Cutting::uniform(0.05));
    assert_ne!(drilled, solid, "a hole changes what the layers hold");

    scene.objects_mut()[0].hollow.clear_drains();
    assert_ne!(
        stack_fingerprint(&scene, Cutting::uniform(0.05)),
        drilled,
        "and so does taking it back out"
    );
}

#[test]
fn a_texture_is_drawn_once_per_layer() {
    let ctx = egui::Context::default();
    let mut preview = preview_with_stack(4);

    let first = preview
        .texture(&ctx, &settings())
        .expect("the stack has layers");
    let again = preview
        .texture(&ctx, &settings())
        .expect("the stack has layers");
    assert_eq!(first.id(), again.id(), "the same layer is not redrawn");

    preview.set_layer(2);
    let moved = preview
        .texture(&ctx, &settings())
        .expect("the stack has layers");
    assert_ne!(moved.id(), first.id(), "another layer is another texture");
    assert_eq!(moved.size(), [64, 32]);
}

#[test]
fn a_cancelled_build_leaves_the_preview_alone() {
    let mut preview = preview_with_stack(2);
    preview
        .build(&scene_with_a_model(), Cutting::uniform(0.5))
        .expect("the plate has a model on it");
    assert!(preview.is_building());

    preview.cancel();
    assert!(!preview.is_building());
    assert_eq!(preview.layer_count(), 2, "the old stack is still shown");
}

/// A `.goo` of `count` layers on a 16 x 8 panel with a lit block in the middle, written
/// to a temporary file so it can be opened by path the way the window opens one.
pub(crate) fn written_goo(name: &str, count: u32) -> std::path::PathBuf {
    use core_format::{LayerSink, PrintJob, SlicedFileWriter};
    use printer_profiles::{MaterialProfile, PrinterProfile};

    let printer = PrinterProfile::from_toml_str(
        r#"
name = "Read Back"
manufacturer = "Test"

[display]
width_px = 16
height_px = 8
width_mm = 1.6
height_mm = 0.8

[build_volume]
x = 1.6
y = 0.8
z = 10.0
"#,
        Path::new("inline.toml"),
    )
    .expect("the inline profile is valid");

    let raster = RasterSettings {
        width_px: 16,
        height_px: 8,
        pitch: PixelPitch { x: 0.1, y: 0.1 },
        ..settings()
    };
    let job = PrintJob {
        printer,
        material: MaterialProfile {
            layer_height_mm: 0.05,
            ..MaterialProfile::default()
        },
        raster,
        plan: LayerPlan::of_count(0.05, count as usize),
        volume_mm3: 0.0,
        exposure: core_format::ExposurePlan::default(),
        thumbnail: None,
        created_unix_s: 0,
    };

    let mut mask = LayerMask::new(16, 8);
    let pixels = mask.pixels_mut();
    for y in 2..6u32 {
        for x in 4..12u32 {
            pixels[(y * 16 + x) as usize] = 255;
        }
    }
    let runs = LayerRuns::from_mask(&mask);

    let path = std::env::temp_dir().join(format!("encrust-preview-{name}.goo"));
    let file = std::fs::File::create(&path).expect("a temporary file");
    let mut buffered = std::io::BufWriter::new(file);
    {
        let mut sink = format_goo::GooWriter
            .begin(&job, &mut buffered)
            .expect("the job matches the panel");
        for _ in 0..count {
            sink.push(format_goo::GooSink::encode(&runs))
                .expect("a layer");
        }
        sink.finish(1234.0).expect("every promised layer arrived");
    }
    path
}
