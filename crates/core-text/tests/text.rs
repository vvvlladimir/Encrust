#![expect(
    clippy::expect_used,
    reason = "a font or a mesh the test built itself cannot fail to read"
)]

use std::sync::Arc;

use core_geometry::{Bvh, Transform, Vec3, cuboid, diagnose, signed_volume};
use core_text::{Font, Frame, LabelSettings, Shape, TextError, label, outline, prism};

fn plex() -> Font {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/fonts/IBMPlexSans-Regular.ttf"
    );
    let data = std::fs::read(path).expect("the window's own font is in assets");
    Font::new(Arc::new(data), 0).expect("IBM Plex Sans is a font")
}

fn height_of(shapes: &[Shape]) -> f32 {
    let ys = shapes
        .iter()
        .flat_map(|shape| shape.outer.iter().map(|point| point.y));
    let (low, high) = ys.fold((f32::MAX, f32::MIN), |(low, high), y| {
        (low.min(y), high.max(y))
    });
    high - low
}

#[test]
fn a_capital_i_is_as_tall_as_the_height_asked_for() {
    let shapes = outline(&plex(), "I", 10.0).expect("I has an outline");
    assert_eq!(shapes.len(), 1);
    assert!(shapes[0].holes.is_empty());
    let height = height_of(&shapes);
    assert!(
        (height - 10.0).abs() < 0.05,
        "a capital stands exactly the capital height; got {height} mm"
    );
}

#[test]
fn letters_with_counters_keep_them_as_holes() {
    let font = plex();
    let o = outline(&font, "O", 10.0).expect("O");
    let b = outline(&font, "B", 10.0).expect("B");
    assert_eq!(
        (o.len(), o[0].holes.len()),
        (1, 1),
        "O is one ring round a hole"
    );
    assert_eq!((b.len(), b[0].holes.len()), (1, 2), "B has two counters");
}

#[test]
fn a_second_line_stands_under_the_first() {
    let font = plex();
    let one = height_of(&outline(&font, "I", 10.0).expect("one line"));
    let two = height_of(&outline(&font, "I\nI", 10.0).expect("two lines"));
    assert!(
        two > 2.0 * one,
        "two lines of 10 mm capitals and a gap; got {two} mm"
    );
}

#[test]
fn a_letter_stands_up_as_a_closed_solid() {
    let shapes = outline(&plex(), "O", 10.0).expect("O");
    let solid = prism(&shapes[0], 0.0, 2.0);
    assert_eq!(diagnose(&solid).boundary_edges, 0, "the prism is closed");
    assert!(signed_volume(&solid) > 0.0, "and wound outward");
}

#[test]
fn bytes_that_are_not_a_font_are_refused() {
    assert!(matches!(
        Font::new(Arc::new(vec![0; 64]), 0),
        Err(TextError::BadFont(_))
    ));
}

#[test]
fn a_character_the_font_lacks_is_named() {
    assert_eq!(
        outline(&plex(), "A\u{1F600}", 10.0),
        Err(TextError::MissingGlyph('\u{1F600}'))
    );
}

#[test]
fn spaces_alone_are_nothing_to_write() {
    assert_eq!(outline(&plex(), "   ", 10.0), Err(TextError::Blank));
}

#[test]
fn a_size_of_nothing_is_refused() {
    assert!(matches!(
        outline(&plex(), "A", 0.0),
        Err(TextError::BadSize { .. })
    ));
}

/// A 40 mm cube on the plate, its top face at z = 40.
fn block() -> (core_geometry::Mesh, Bvh) {
    let mesh = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    let bvh = Bvh::build(&mesh);
    (mesh, bvh)
}

#[test]
fn a_raised_label_stands_its_depth_out_of_a_flat_face_and_bites_into_it() {
    let (mesh, bvh) = block();
    let shapes = outline(&plex(), "AB", 8.0).expect("AB");
    let frame = Frame::on(Vec3::new(20.0, 20.0, 40.0), Vec3::Z, 0.0).expect("a frame");
    for (raised, low, high) in [(true, 39.8, 41.5), (false, 38.5, 40.2)] {
        let label = label(
            &shapes,
            frame,
            &mesh,
            &bvh,
            Transform::default(),
            LabelSettings {
                depth_mm: 1.5,
                raised,
            },
        )
        .expect("the label stands on the cube");
        let zs = label
            .prisms
            .iter()
            .flat_map(|prism| prism.vertices.iter().map(|v| v.z));
        let (bottom, top) = zs.fold((f32::MAX, f32::MIN), |(b, t), z| (b.min(z), t.max(z)));
        assert!(
            (bottom - low).abs() < 1e-3 && (top - high).abs() < 1e-3,
            "raised {raised}: 1.5 mm off a face at 40 mm, 0.2 mm into the other side; got {bottom}..{top}"
        );
    }
}

#[test]
fn a_label_beside_the_model_stands_on_nothing() {
    let (mesh, bvh) = block();
    let shapes = outline(&plex(), "A", 8.0).expect("A");
    let frame = Frame::on(Vec3::new(200.0, 200.0, 40.0), Vec3::Z, 0.0).expect("a frame");
    let settings = LabelSettings {
        depth_mm: 1.0,
        raised: true,
    };
    assert_eq!(
        label(&shapes, frame, &mesh, &bvh, Transform::default(), settings),
        Err(TextError::OffTheModel)
    );
    assert!(matches!(
        label(
            &shapes,
            frame,
            &mesh,
            &bvh,
            Transform::default(),
            LabelSettings {
                depth_mm: -1.0,
                raised: true
            }
        ),
        Err(TextError::BadSize { .. })
    ));
}

#[test]
fn a_label_on_a_wall_reads_up_the_plate() {
    let frame = Frame::on(Vec3::ZERO, Vec3::X, 0.0).expect("a frame");
    assert!(
        (frame.up - Vec3::Z).length() < 1e-6,
        "up a side wall is up the plate"
    );
    let turned = Frame::on(Vec3::ZERO, Vec3::X, 90.0).expect("a frame");
    assert!(
        turned.up.dot(Vec3::Z).abs() < 1e-6,
        "a quarter turn lays it on its side"
    );
}

#[test]
fn a_label_beside_a_pillar_keeps_to_its_depth() {
    // A 40 mm plate with a 20 mm pillar standing on its right half, under the label.
    let mut mesh = cuboid(Vec3::ZERO, Vec3::new(40.0, 40.0, 4.0));
    mesh.append(&cuboid(
        Vec3::new(20.0, 15.0, 4.0),
        Vec3::new(24.0, 25.0, 24.0),
    ));
    let bvh = Bvh::build(&mesh);
    let shapes = outline(&plex(), "WIDE", 8.0).expect("WIDE");
    let frame = Frame::on(Vec3::new(20.0, 20.0, 4.0), Vec3::Z, 0.0).expect("a frame");
    for raised in [true, false] {
        let settings = LabelSettings {
            depth_mm: 0.5,
            raised,
        };
        let label = label(&shapes, frame, &mesh, &bvh, Transform::default(), settings)
            .expect("the label stands on the plate");
        let top = label
            .prisms
            .iter()
            .flat_map(|prism| prism.vertices.iter().map(|v| v.z))
            .fold(f32::MIN, f32::max);
        assert!(
            top <= 4.0 + 1.0 + 1e-3,
            "raised {raised}: never more than twice its depth off the plate at 4 mm; got {top}"
        );
    }
}
