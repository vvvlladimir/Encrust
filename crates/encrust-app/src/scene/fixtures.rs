//! Plates the tests stand models on.

use std::sync::Arc;

use core_geometry::{Transform, Vec3, cuboid, diagnose};

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
