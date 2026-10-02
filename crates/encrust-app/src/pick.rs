use core_geometry::{Mat4, PlacedHit, Ray, Scalar, Vec3, raycast_placed};

use crate::camera::OrbitCamera;
use crate::scene::{ObjectId, Scene};

/// The ray under the cursor, in plate coordinates.
///
/// `viewport` is the rectangle the 3D view was drawn into and `cursor` a position inside
/// it, both in egui points. Returns `None` for a viewport with no area.
pub fn ray_through(camera: &OrbitCamera, viewport: egui::Rect, cursor: egui::Pos2) -> Option<Ray> {
    if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
        return None;
    }

    let x = 2.0 * (cursor.x - viewport.left()) / viewport.width() - 1.0;
    let y = 1.0 - 2.0 * (cursor.y - viewport.top()) / viewport.height();

    let inverse = camera
        .view_projection(viewport.width() / viewport.height())
        .inverse();

    // wgpu clip space runs from 0 at the near plane to 1 at the far plane, unlike
    // OpenGL's -1 to 1. See docs/design/viewport.md.
    let near = unproject(&inverse, Vec3::new(x, y, 0.0))?;
    let far = unproject(&inverse, Vec3::new(x, y, 1.0))?;

    Some(Ray::new(near, far - near))
}

fn unproject(inverse_view_projection: &Mat4, clip: Vec3) -> Option<Vec3> {
    let point = *inverse_view_projection * clip.extend(1.0);
    (point.w.abs() > Scalar::EPSILON).then(|| point.truncate() / point.w)
}

/// The visible object nearest the camera under the cursor, if any.
pub fn pick(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<ObjectId> {
    pick_surface(scene, camera, viewport, cursor).map(|(id, _)| id)
}

/// The visible object nearest the camera under the cursor, and where the ray met it, in
/// plate coordinates. What the Supports tool puts a support on.
pub fn pick_surface(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(ObjectId, PlacedHit)> {
    let ray = ray_through(camera, viewport, cursor)?;

    scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            let hit = raycast_placed(&object.mesh, &object.bvh, object.transform, &ray)?;
            Some((object.id, hit))
        })
        .min_by(|(_, a), (_, b)| a.distance.total_cmp(&b.distance))
}

/// Whether a visible model stands between `eye` and `point`, so that `point` is hidden
/// from a camera there. A surface `point` itself lies on does not count.
pub fn occluded(scene: &Scene, eye: Vec3, point: Vec3) -> bool {
    let reach = point - eye;
    let ray = Ray::new(eye, reach);
    // Bounds touch the model they box, so a hit this close to `point` is that touch.
    let short_of = reach.length() * (1.0 - 1e-3) - 1e-3;
    scene.printable(scene.active_plate()).any(|object| {
        raycast_placed(&object.mesh, &object.bvh, object.transform, &ray)
            .is_some_and(|hit| hit.distance < short_of)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Mesh, Orientation, Transform, diagnose};
    use std::sync::Arc;

    const VIEWPORT: egui::Rect = egui::Rect {
        min: egui::Pos2::new(0.0, 0.0),
        max: egui::Pos2::new(800.0, 600.0),
    };

    fn center() -> egui::Pos2 {
        VIEWPORT.center()
    }

    /// Axis-aligned cube spanning 0..1 on every axis.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
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

    fn summary() -> ImportSummary {
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&unit_cube()),
        }
    }

    fn add(scene: &mut Scene, name: &str, at: Vec3) -> ObjectId {
        scene.insert(crate::scene::Imported::new(
            name.to_owned(),
            Arc::new(unit_cube()),
            Transform::from_translation(at),
            summary(),
        ))
    }

    /// A camera whose target is the middle of the cube placed at `at`.
    fn looking_at(at: Vec3) -> OrbitCamera {
        OrbitCamera {
            target: at + Vec3::splat(0.5),
            distance_mm: 20.0,
            ..OrbitCamera::default()
        }
    }

    #[test]
    fn the_centre_of_the_viewport_hits_what_the_camera_looks_at() {
        let mut scene = Scene::default();
        let id = add(&mut scene, "cube", Vec3::ZERO);
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(pick(&scene, &camera, VIEWPORT, center()), Some(id));
    }

    #[test]
    fn the_nearer_of_two_stacked_objects_wins() {
        let mut scene = Scene::default();
        let far = add(&mut scene, "far", Vec3::ZERO);
        let camera = looking_at(Vec3::ZERO);

        // A second cube centred 5 mm from the target towards the eye, so it sits
        // squarely in front of the first one.
        let towards_eye = (camera.eye() - camera.target).normalize() * 5.0;
        let near = add(&mut scene, "near", towards_eye);

        let picked = pick(&scene, &camera, VIEWPORT, center());
        assert_eq!(picked, Some(near));
        assert_ne!(picked, Some(far));
    }

    #[test]
    fn a_corner_of_the_viewport_hits_nothing() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(pick(&scene, &camera, VIEWPORT, VIEWPORT.min), None);
    }

    #[test]
    fn a_hidden_object_cannot_be_picked() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        scene.objects_mut()[0].visible = false;
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(pick(&scene, &camera, VIEWPORT, center()), None);
    }

    #[test]
    fn a_flattened_object_cannot_be_picked() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        scene.objects_mut()[0].transform.scale = Vec3::new(1.0, 1.0, 0.0);
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(pick(&scene, &camera, VIEWPORT, center()), None);
    }

    #[test]
    fn a_viewport_with_no_area_produces_no_ray() {
        let camera = OrbitCamera::default();
        let empty = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
        assert!(ray_through(&camera, empty, egui::Pos2::ZERO).is_none());
    }

    #[test]
    fn the_ray_through_the_centre_runs_down_the_view_axis() {
        let camera = looking_at(Vec3::ZERO);
        let ray = ray_through(&camera, VIEWPORT, center()).expect("the viewport has area");
        let axis = (camera.target - camera.eye()).normalize();
        assert!(
            ray.direction.abs_diff_eq(axis, 1e-4),
            "expected the view axis {axis}, got {}",
            ray.direction
        );
    }

    #[test]
    fn a_point_behind_a_model_is_hidden_and_one_beside_it_is_not() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        let eye = Vec3::new(0.5, 0.5, 10.0);
        assert!(
            occluded(&scene, eye, Vec3::new(0.5, 0.5, -5.0)),
            "the unit cube stands between the eye and a point under it"
        );
        assert!(!occluded(&scene, eye, Vec3::new(5.0, 0.5, -5.0)));
        assert!(
            !occluded(&scene, eye, Vec3::new(0.5, 0.5, 1.0)),
            "a point on the cube's own top face is not hidden by it"
        );
    }
}
