use super::*;
use crate::settings::Rgb;

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
    let behind = Transform::from_translation(EYE.normalize() * -4.0);
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

#[test]
fn a_ball_is_shaded_without_steps_between_its_facets() {
    let mesh = core_geometry::ball(Vec3::ZERO, 1.0, 12, 16);
    let image = render(&[Part::placed(&mesh)], &settings());
    let row = &image.pixels()[32 * 64 + 16..32 * 64 + 48];
    let steepest = row
        .windows(2)
        .map(|pair| pair[0][2].abs_diff(pair[1][2]))
        .max()
        .expect("thirty-two pixels");
    assert!(
        steepest <= 12,
        "across the middle of a ball of 16 facets the blue changes by at most 12 a pixel, \
         got {steepest}"
    );
}
