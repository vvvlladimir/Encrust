use super::*;
use crate::scene::{ImportSummary, Imported};
use core_geometry::{Aabb, Mesh, Orientation, Transform, diagnose};
use std::sync::Arc;

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

fn plate() -> BuildPlate {
    BuildPlate {
        name: None,
        x_mm: 150.0,
        y_mm: 80.0,
        z_mm: 165.0,
        ..BuildPlate::default()
    }
}

/// Three boxes stacked on the same spot, which is what dropping three files gives.
fn heap(count: usize) -> Scene {
    let mesh = Arc::new(cuboid(Vec3::new(20.0, 20.0, 10.0)));
    let mut scene = Scene::default();
    for index in 0..count {
        scene.insert(Imported::new(
            format!("box {index}"),
            Arc::clone(&mesh),
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
    }
    scene
}

fn overlap(a: &Aabb, b: &Aabb) -> bool {
    a.mins.x < b.maxs.x && b.mins.x < a.maxs.x && a.mins.y < b.maxs.y && b.mins.y < a.maxs.y
}

#[test]
fn a_heap_of_models_is_spread_out_over_the_plate() {
    let mut scene = heap(3);
    let status = arrange_plate(&mut scene, &plate(), 3.0);
    assert!(!status.is_error());

    let bounds: Vec<Aabb> = scene
        .objects()
        .iter()
        .filter_map(|object| object.world_bounds())
        .collect();
    assert_eq!(bounds.len(), 3);
    for (index, first) in bounds.iter().enumerate() {
        for second in &bounds[index + 1..] {
            assert!(
                !overlap(first, second),
                "{first:?} still overlaps {second:?}"
            );
        }
    }
}

/// Where a mesh in plate coordinates stands across the plate, as an `Aabb`.
fn bounds_of(mesh: &Mesh) -> Aabb {
    let mut bounds = Aabb::new(Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for vertex in &mesh.vertices {
        bounds.mins = bounds.mins.min(*vertex);
        bounds.maxs = bounds.maxs.max(*vertex);
    }
    bounds
}

/// Builds every object's supports where it now stands.
fn rebuild(scene: &mut Scene) {
    let table = vec![printer_profiles::SupportProfile::default()];
    for object in scene.objects_mut() {
        let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
        object
            .supports
            .refresh(&mesh, &bvh, object.transform, &table);
    }
}

#[test]
fn a_support_pad_out_past_its_model_keeps_clear_of_the_next_model() {
    let mut scene = heap(3);
    for object in scene.objects_mut() {
        object.transform.translation.z = 10.0;
        let transform = object.transform;
        // Under the corner, so half the pad stands out past the box.
        object.supports.add(Vec3::new(0.5, 0.5, 10.0), transform, 0);
    }
    rebuild(&mut scene);
    arrange_plate(&mut scene, &plate(), 0.5);
    rebuild(&mut scene);

    let objects = scene.objects();
    for (index, object) in objects.iter().enumerate() {
        let pads: Vec<Aabb> = object
            .supports
            .drawn()
            .iter()
            .filter(|group| !group.mesh.is_empty())
            .map(|group| bounds_of(&group.mesh))
            .collect();
        assert!(!pads.is_empty(), "the support was built");
        for other in objects.iter().enumerate().filter(|(at, _)| *at != index) {
            let model = other.1.world_bounds().expect("a box has bounds");
            for pad in &pads {
                assert!(
                    !overlap(pad, &model),
                    "the support of {} at {pad:?} stands under {}",
                    object.name,
                    other.1.name
                );
            }
        }
    }
}

#[test]
fn arranging_leaves_the_models_standing_at_the_height_they_were() {
    let mut scene = heap(2);
    scene.objects_mut()[0].transform.translation.z = 5.0;
    arrange_plate(&mut scene, &plate(), 3.0);
    assert_eq!(scene.objects()[0].transform.translation.z, 5.0);
}

#[test]
fn an_empty_plate_is_not_a_failure() {
    let mut scene = Scene::default();
    let status = arrange_plate(&mut scene, &plate(), 3.0);
    assert!(!status.is_error());
}

#[test]
fn a_model_too_big_for_the_plate_is_reported_rather_than_moved() {
    let mut scene = Scene::default();
    let mesh = Arc::new(cuboid(Vec3::new(300.0, 20.0, 10.0)));
    scene.insert(Imported::new(
        "slab".to_owned(),
        Arc::clone(&mesh),
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

    let status = arrange_plate(&mut scene, &plate(), 3.0);
    assert!(!status.is_error(), "not fitting is news, not a failure");
    assert_eq!(scene.objects()[0].transform.translation, Vec3::ZERO);
}
