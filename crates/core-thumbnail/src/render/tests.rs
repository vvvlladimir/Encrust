use super::*;

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

fn settings() -> ThumbnailSettings {
    ThumbnailSettings {
        width_px: 64,
        height_px: 64,
        ..ThumbnailSettings::default()
    }
}

#[test]
fn an_empty_plate_renders_to_the_background() {
    let settings = settings();
    let image = render(&[], &settings);
    assert!(
        image
            .pixels()
            .iter()
            .all(|pixel| *pixel == settings.background)
    );
}

#[test]
fn a_cube_covers_the_middle_and_leaves_the_corners_empty() {
    let mesh = unit_cube();
    let settings = settings();
    let image = render(&[Part::placed(&mesh)], &settings);

    let middle = image.pixels()[(32 * 64 + 32) as usize];
    assert_ne!(middle, settings.background, "the part covers the centre");
    for corner in [0usize, 63, 63 * 64, 64 * 64 - 1] {
        assert_eq!(
            image.pixels()[corner],
            settings.background,
            "the margin keeps the part off the border"
        );
    }
}

#[test]
fn three_faces_of_a_cube_are_shaded_differently() {
    let mesh = unit_cube();
    let image = render(&[Part::placed(&mesh)], &settings());

    let mut shades: Vec<Rgb> = image.pixels().to_vec();
    shades.sort_unstable();
    shades.dedup();
    assert!(
        shades.len() >= 4,
        "background plus one shade per visible face, got {}",
        shades.len()
    );
}

#[test]
fn the_view_frames_the_part_wherever_it_stands() {
    let mesh = unit_cube();
    let settings = settings();
    let here = render(&[Part::placed(&mesh)], &settings);
    let there = render(
        &[Part::new(
            &mesh,
            Transform::from_translation(Vec3::new(40.0, -25.0, 3.0)),
        )],
        &settings,
    );

    assert_eq!(
        here, there,
        "the frame follows the part, so moving it on the plate changes nothing"
    );
}

#[test]
fn a_nearer_part_hides_the_one_behind_it() {
    let mesh = unit_cube();
    let settings = ThumbnailSettings {
        model: [255, 255, 255],
        ..settings()
    };
    // The second cube sits directly behind the first along the view direction, so the
    // image must look exactly like the front one alone.
    let behind = Transform::from_translation(View::Iso.basis().forward * 4.0);
    let alone = render(&[Part::placed(&mesh)], &settings);
    let stacked = render(&[Part::placed(&mesh), Part::new(&mesh, behind)], &settings);

    let lit = |image: &Thumbnail| {
        image
            .pixels()
            .iter()
            .filter(|pixel| **pixel != settings.background)
            .count()
    };
    assert_eq!(
        lit(&alone),
        lit(&stacked),
        "the far cube is hidden behind the near one"
    );
}

const RED: Rgb = [0xFF, 0x30, 0x20];

fn square(px: u32) -> ThumbnailSettings {
    ThumbnailSettings {
        width_px: px,
        height_px: px,
        ..ThumbnailSettings::default()
    }
}

fn marked(at: Vec3) -> Marker {
    Marker { at, colour: RED }
}

fn pixel(image: &Thumbnail, x: u32, y: u32) -> Rgb {
    image.pixels()[(y * image.width_px() + x) as usize]
}

/// The least and greatest column and row the part covers.
fn covered(image: &Thumbnail, background: Rgb) -> (u32, u32, u32, u32) {
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..image.height_px() {
        for x in 0..image.width_px() {
            if pixel(image, x, y) != background {
                (left, top) = (left.min(x), top.min(y));
                (right, bottom) = (right.max(x), bottom.max(y));
            }
        }
    }
    (left, top, right, bottom)
}

#[test]
fn a_unit_cube_seen_from_the_top_fills_the_framed_square() {
    let mesh = unit_cube();
    let settings = square(128);
    let image = render_view(&[Part::placed(&mesh)], View::Top, &[], &settings).image;

    let lit = image
        .pixels()
        .iter()
        .filter(|pixel| **pixel != settings.background)
        .count() as f32;
    // The top face alone, filling the image less a 6% margin on every side: (0.88 * 128)^2.
    let expected = (0.88_f32 * 128.0).powi(2);
    assert!(
        (lit - expected).abs() <= expected * 0.02,
        "expected about {expected} lit pixels, got {lit}"
    );
}

#[test]
fn a_two_by_one_box_seen_from_the_front_is_twice_as_wide_as_tall() {
    let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::new(2.0, 1.0, 1.0));
    let settings = square(128);
    let image = render_view(&[Part::placed(&mesh)], View::Front, &[], &settings).image;

    let (left, top, right, bottom) = covered(&image, settings.background);
    let aspect = (right - left + 1) as f32 / (bottom - top + 1) as f32;
    assert!(
        (aspect - 2.0).abs() < 0.05,
        "expected a silhouette 2 wide by 1 tall, got {aspect}"
    );
}

#[test]
fn a_marker_lands_on_the_pixel_its_plate_point_projects_to() {
    let mesh = unit_cube();
    let rendered = render_view(
        &[Part::placed(&mesh)],
        View::Top,
        &[marked(Vec3::new(0.75, 0.25, 1.0))],
        &square(128),
    );

    // A quarter of the cube right of and in front of its middle, at 0.88 * 128 px per mm.
    let expected = 64.0 + 0.25 * 0.88 * 128.0;
    let spot = rendered.markers[0];
    assert!(
        (spot.x_px - expected).abs() < 0.5 && (spot.y_px - expected).abs() < 0.5,
        "expected the marker at ({expected}, {expected}), got ({}, {})",
        spot.x_px,
        spot.y_px
    );
    assert!(!spot.hidden, "the point is on the face the eye sees");
    assert_eq!(
        pixel(&rendered.image, expected as u32, expected as u32),
        RED
    );
}

#[test]
fn a_marker_under_the_model_is_a_ring_from_above_and_a_disc_from_below() {
    let mesh = unit_cube();
    let settings = square(256);
    let under = [marked(Vec3::new(0.5, 0.5, 0.0))];
    let from_top = render_view(&[Part::placed(&mesh)], View::Top, &under, &settings);
    let from_below = render_view(&[Part::placed(&mesh)], View::Bottom, &under, &settings);

    assert!(from_top.markers[0].hidden, "the cube stands over the point");
    let radius = marker::radius_px(256, 256);
    let on_the_ring = 128 + radius as u32 - 1;
    assert_eq!(
        pixel(&from_top.image, on_the_ring, 128),
        RED,
        "the ring is drawn over the model, {radius} px out"
    );
    assert_ne!(
        pixel(&from_top.image, 128, 128),
        RED,
        "the ring is hollow, so the model shows inside it"
    );

    assert!(
        !from_below.markers[0].hidden,
        "the point is on the bottom face"
    );
    assert_eq!(pixel(&from_below.image, 128, 128), RED);
}

#[test]
fn each_side_view_shows_the_front_left_top_corner_where_that_side_puts_it() {
    let mesh = unit_cube();
    // Whether the corner at (0, 0, 1) is on the left half and on the upper half of the image.
    let expected = [
        (View::Front, true, true),
        (View::Back, false, true),
        (View::Left, false, true),
        (View::Right, true, true),
        (View::Top, true, false),
        (View::Bottom, true, true),
    ];
    for (view, left, upper) in expected {
        let rendered = render_view(
            &[Part::placed(&mesh)],
            view,
            &[marked(Vec3::new(0.0, 0.0, 1.0))],
            &square(64),
        );
        let spot = rendered.markers[0];
        assert_eq!(
            (spot.x_px < 32.0, spot.y_px < 32.0),
            (left, upper),
            "from the {}, expected left {left} and upper {upper}, got ({}, {})",
            view.name(),
            spot.x_px,
            spot.y_px
        );
    }
}

#[test]
fn a_marker_far_off_the_part_is_pinned_to_the_edge_and_leaves_the_part_its_size() {
    let mesh = unit_cube();
    let rendered = render_view(
        &[Part::placed(&mesh)],
        View::Top,
        &[marked(Vec3::new(1000.0, 0.5, 1.0))],
        &square(128),
    );

    // The frame spans the 1 mm cube alone, so its middle is still the picture's middle.
    let middle = rendered.image.pixels()[64 * 128 + 64];
    assert_ne!(
        middle,
        square(128).background,
        "the cube still fills the middle"
    );
    let spot = rendered.markers[0];
    let radius = marker::radius_px(128, 128);
    assert!(spot.pinned, "a point a metre off lies off the picture");
    assert!(
        (spot.x_px - (128.0 - radius)).abs() < 1e-3 && (spot.y_px - 64.0).abs() < 1.0,
        "expected it on the right edge, a radius in, at ({}, 64), got ({}, {})",
        128.0 - radius,
        spot.x_px,
        spot.y_px
    );
}

#[test]
fn a_marker_with_no_part_frames_the_picture_itself() {
    let rendered = render_view(
        &[],
        View::Top,
        &[marked(Vec3::new(5.0, 5.0, 0.0))],
        &square(64),
    );
    let spot = rendered.markers[0];
    assert!(
        !spot.pinned,
        "on an empty plate the marks are what is framed"
    );
    assert!((spot.x_px - 32.0).abs() < 1e-3 && (spot.y_px - 32.0).abs() < 1e-3);
}
