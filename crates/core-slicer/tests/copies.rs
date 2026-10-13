#[path = "shared/bodies.rs"]
mod shared;

use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3, transform_mesh};
use core_slicer::{Contour, Repeated, SliceSettings, Solid, Source, Windows};
use shared::bodies;

fn appended(mut whole: Mesh, part: &Mesh) -> Mesh {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
    whole
}

/// Each contour as its area and the middle of its points, in a fixed order.
fn summary(contours: &[Contour]) -> Vec<(Scalar, Vec2)> {
    let mut summary: Vec<(Scalar, Vec2)> = contours
        .iter()
        .map(|contour| {
            let sum: Vec2 = contour.points.iter().copied().sum();
            (contour.area(), sum / contour.points.len() as Scalar)
        })
        .collect();
    summary.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.1.y.total_cmp(&b.1.y)));
    summary
}

#[test]
fn a_copy_laid_down_again_matches_the_copy_cut_where_it_stands() {
    let ball = transform_mesh(
        &bodies::uv_sphere(5.0, 24, 24),
        Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
    );
    let offset = Vec2::new(20.0, 7.0);
    let moved = transform_mesh(
        &ball,
        Transform::from_translation(Vec3::new(offset.x, offset.y, 0.0)),
    );
    let whole = appended(ball.clone(), &moved);
    let solid = Solid {
        mesh: Mesh::default(),
        repeated: vec![Repeated {
            mesh: ball,
            offsets_mm: vec![offset],
        }],
    };
    let windows = Windows::new(
        &solid.distinct(),
        SliceSettings {
            layer_height: 0.5,
            ..SliceSettings::default()
        },
        64,
    )
    .expect("a ball plans");

    let cut = windows
        .cut(&whole, 0..windows.layer_count())
        .expect("two balls slice");
    let stamped = windows
        .cut(&solid, 0..windows.layer_count())
        .expect("one ball slices");
    assert_eq!(stamped.layers.len(), cut.layers.len());
    for (stamped, cut) in stamped.layers.iter().zip(&cut.layers) {
        let (stamped, cut) = (summary(&stamped.contours), summary(&cut.contours));
        assert_eq!(
            stamped.len(),
            cut.len(),
            "one contour a ball on every layer"
        );
        for ((area, middle), (cut_area, cut_middle)) in stamped.iter().zip(&cut) {
            assert!(
                (area - cut_area).abs() < 1e-3 && middle.distance(*cut_middle) < 1e-4,
                "a ball moved is the same ball, moved: {area} at {middle} against {cut_area} at {cut_middle}"
            );
        }
    }
}

#[test]
fn a_plate_spans_every_copy() {
    let solid = Solid {
        mesh: Mesh::default(),
        repeated: vec![Repeated {
            mesh: bodies::cube(1.0),
            offsets_mm: vec![Vec2::new(10.0, 0.0), Vec2::new(0.0, 5.0)],
        }],
    };
    let bounds = solid.aabb().expect("a cube has bounds");
    assert!(
        bounds.maxs.abs_diff_eq(Vec3::new(11.0, 6.0, 1.0), 1e-6),
        "a unit cube moved 10 along x and 5 along y reaches 11 and 6, got {}",
        bounds.maxs
    );
}

#[test]
fn what_a_broken_mesh_forces_is_counted_once_a_copy() {
    // A cube with one triangle of a side wall gone: every plane through the wall finds
    // a chain that runs out of faces.
    let mut open = bodies::cube(2.0);
    open.faces.remove(4);
    let one = open.slice_at(&[1.0]).expect("a torn cube still slices");
    assert!(one.open_contours > 0, "the wall is torn");

    let solid = Solid {
        mesh: Mesh::default(),
        repeated: vec![Repeated {
            mesh: open,
            offsets_mm: vec![Vec2::new(5.0, 0.0), Vec2::new(10.0, 0.0)],
        }],
    };
    let three = solid.slice_at(&[1.0]).expect("a torn cube still slices");
    assert_eq!(three.open_contours, 3 * one.open_contours);
}
