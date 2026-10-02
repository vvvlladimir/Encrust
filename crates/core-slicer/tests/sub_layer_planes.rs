#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::num::NonZeroU8;

use core_geometry::{Mesh, Scalar, Vec3};
use core_slicer::{ONE_SAMPLE, SliceSettings, Windows};

/// An axis-aligned box, wound outward.
fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
    let v = |x: Scalar, y: Scalar, z: Scalar| Vec3::new(x, y, z);
    let vertices = vec![
        v(min.x, min.y, min.z),
        v(max.x, min.y, min.z),
        v(max.x, max.y, min.z),
        v(min.x, max.y, min.z),
        v(min.x, min.y, max.z),
        v(max.x, min.y, max.z),
        v(max.x, max.y, max.z),
        v(min.x, max.y, max.z),
    ];
    let faces = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    Mesh::new(vertices, faces)
}

/// A 10 mm slab one layer thick, with a fin standing in the top fifth of that layer only.
fn slab_with_a_high_fin() -> Mesh {
    let mut mesh = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 1.0));
    let fin = box_mesh(Vec3::new(20.0, 4.0, 0.8), Vec3::new(21.0, 6.0, 0.95));
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(fin.vertices);
    mesh.faces
        .extend(fin.faces.iter().map(|f| f.map(|i| base + i)));
    mesh
}

fn cut(samples: NonZeroU8) -> core_slicer::Layer {
    let mesh = slab_with_a_high_fin();
    let windows = Windows::new(
        &mesh,
        SliceSettings {
            layer_height: 1.0,
            samples,
        },
        64,
    )
    .expect("the slab slices");
    assert_eq!(windows.layer_count(), 1, "the slab is one layer tall");

    let mut sliced = windows.cut(&mesh, 0..1).expect("the layer cuts");
    sliced.layers.pop().expect("one layer came back")
}

#[test]
fn one_plane_a_layer_never_sees_a_fin_the_plane_misses() {
    let layer = cut(ONE_SAMPLE);
    assert!((layer.z - 0.5).abs() < 1e-6, "the middle of the only band");
    assert_eq!(layer.contours.len(), 1, "the slab, and nothing else");
    assert!(layer.extra.is_empty());
}

#[test]
fn three_planes_a_layer_find_a_fin_thinner_than_the_layer() {
    let layer = cut(NonZeroU8::new(3).expect("three is not zero"));

    assert!(
        (layer.z - 0.5).abs() < 1e-6,
        "the plate still stands at the middle of the band"
    );
    assert_eq!(
        layer.contours.len(),
        1,
        "the plane nearest the middle leads, and it only meets the slab"
    );
    assert_eq!(layer.extra.len(), 2, "two more planes, at 1/6 and 5/6");

    // Only the plane at z = 0.833 crosses the fin, which sits from 0.8 to 0.95.
    let planes_with_the_fin = layer
        .extra
        .iter()
        .filter(|contours| contours.len() == 2)
        .count();
    assert_eq!(planes_with_the_fin, 1);
}
