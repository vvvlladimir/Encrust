//! Plates the tests stand models on.

use std::sync::Arc;

use core_geometry::{Transform, Vec3, cuboid, diagnose};
use core_volume::{Cancel, HollowSettings, Shell, hollow};

use super::{ImportSummary, Imported, Scene};

/// A plate of boxes, each from its low to its high corner, plate millimetres.
pub fn scene_of_boxes(boxes: &[(Vec3, Vec3)]) -> Scene {
    let mut scene = Scene::default();
    for (low, high) in boxes {
        let mesh = cuboid(*low, *high);
        let summary = ImportSummary::made(diagnose(&mesh));
        scene.insert(Imported::new(
            "box".to_owned(),
            Arc::new(mesh),
            Transform::default(),
            summary,
        ));
    }
    scene
}

/// A cube `side_mm` across from the origin, hollowed sealed to `wall_mm` as Hollow cuts it,
/// standing where `transform` puts it: turned or moved, never scaled, so its wall is the plate's.
pub fn hollow_cube(side_mm: f32, wall_mm: f32, transform: Transform) -> Scene {
    let mut scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::splat(side_mm))]);
    let object = &mut scene.objects_mut()[0];
    object.transform = transform;
    let settings = HollowSettings::for_wall(wall_mm);
    let hollowed = hollow(&object.mesh, &object.bvh, &settings, Cancel::never())
        .expect("a closed cube hollows");
    object.hollow.take(Shell {
        mesh: Arc::new(hollowed.mesh),
        cavity: hollowed.cavity,
        cavity_mm3: hollowed.cavity_mm3,
        voxel_mm: hollowed.voxel_mm,
        coarsened: hollowed.coarsened,
        shape: transform.unturned(),
        settings,
    });
    scene
}
