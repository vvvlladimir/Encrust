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

/// The heights the section cut leaves drawn, plate millimetres; a bound of `None` leaves
/// the model whole on that side. What stands outside them cannot be pointed at.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shown {
    pub floor_mm: Option<Scalar>,
    pub top_mm: Option<Scalar>,
}

impl Shown {
    /// The stretch of `ray` between the cuts, as distances along it, or `None` when it
    /// never passes between them.
    fn span(self, ray: &Ray) -> Option<(Scalar, Scalar)> {
        let (mut near, mut far) = (0.0, Scalar::INFINITY);
        for (bound, kept_above) in [(self.floor_mm, true), (self.top_mm, false)] {
            let Some(height) = bound else {
                continue;
            };
            let rise = height - ray.origin.z;
            if ray.direction.z.abs() <= Scalar::EPSILON {
                if (rise > 0.0) == kept_above && rise != 0.0 {
                    return None;
                }
                continue;
            }
            let t = rise / ray.direction.z;
            if (ray.direction.z > 0.0) == kept_above {
                near = Scalar::max(near, t);
            } else {
                far = Scalar::min(far, t);
            }
        }
        (near <= far).then_some((near, far))
    }
}

/// The stretch of `ray` a support can be seen along, as distances: from where the section
/// lets the ray in to the first thing drawn that hides what is behind it — a model's
/// printed surface, its cavity and holes included, or the cap the cut leaves across its
/// material. `None` when the cuts leave the ray nothing.
pub fn seen_along(scene: &Scene, ray: &Ray, shown: Shown) -> Option<(Scalar, Scalar)> {
    let (near, far) = shown.span(ray)?;
    let from_cut = Ray::new(ray.at(near), ray.direction);
    let wall = scene
        .here()
        .filter_map(|object| {
            let (surface, bvh) = object.support_surface();
            let hit = raycast_placed(surface, bvh, object.transform, &from_cut)?;
            // A first face the ray leaves through: the cut stands in material, capped.
            let capped = near > 0.0 && hit.normal.dot(ray.direction) > 0.0;
            Some(if capped { near } else { near + hit.distance })
        })
        .fold(far, Scalar::min);
    Some((near, wall))
}

/// What the cursor is over where the section cut a model open: one of its faces, or the
/// cap the cut leaves across its material, which has no face of its own.
enum Under {
    Face(ObjectId, PlacedHit),
    Cap(ObjectId, PlacedHit),
}

/// The visible object nearest the camera under the cursor, if any. The cap a section cut
/// leaves across a model is that model.
pub fn pick(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    shown: Shown,
) -> Option<ObjectId> {
    pick_material(scene, camera, viewport, cursor, shown).map(|(id, _)| id)
}

/// The visible object nearest the camera under the cursor, and where the ray met it, in
/// plate coordinates. Only a drawn face counts: none the section cut away, nor its cap.
pub fn pick_surface(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    shown: Shown,
) -> Option<(ObjectId, PlacedHit)> {
    match under(scene, camera, viewport, cursor, shown)? {
        Under::Face(id, hit) => Some((id, hit)),
        Under::Cap(..) => None,
    }
}

/// As [`pick_surface`], but the cap across a cut counts too, met where the ray crosses the
/// cut with the cut's own normal: what a blocker is put inside the material through.
pub fn pick_material(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    shown: Shown,
) -> Option<(ObjectId, PlacedHit)> {
    match under(scene, camera, viewport, cursor, shown)? {
        Under::Face(id, hit) | Under::Cap(id, hit) => Some((id, hit)),
    }
}

fn under(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    shown: Shown,
) -> Option<Under> {
    let ray = ray_through(camera, viewport, cursor)?;
    let (near, far) = shown.span(&ray)?;
    let from_cut = Ray::new(ray.at(near), ray.direction);

    let hits: Vec<(ObjectId, PlacedHit)> = scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            let mut hit = raycast_placed(&object.mesh, &object.bvh, object.transform, &from_cut)?;
            hit.distance += near;
            Some((object.id, hit))
        })
        .collect();

    // A model whose first face past the cut is one the ray leaves it through holds the cut
    // point in its material, where the cap is drawn in front of everything behind it.
    let cut_open = hits
        .iter()
        .find(|(_, hit)| near > 0.0 && hit.normal.dot(ray.direction) > 0.0);
    if let Some((id, hit)) = cut_open {
        let cap = PlacedHit {
            point: from_cut.origin,
            normal: Vec3::new(0.0, 0.0, -ray.direction.z.signum()),
            distance: near,
            ..*hit
        };
        return Some(Under::Cap(*id, cap));
    }
    hits.into_iter()
        .filter(|(_, hit)| hit.distance <= far)
        .min_by(|(_, a), (_, b)| a.distance.total_cmp(&b.distance))
        .map(|(id, hit)| Under::Face(id, hit))
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
        assert_eq!(
            pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
            Some(id)
        );
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

        let picked = pick(&scene, &camera, VIEWPORT, center(), Shown::default());
        assert_eq!(picked, Some(near));
        assert_ne!(picked, Some(far));
    }

    #[test]
    fn a_corner_of_the_viewport_hits_nothing() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(
            pick(&scene, &camera, VIEWPORT, VIEWPORT.min, Shown::default()),
            None
        );
    }

    #[test]
    fn a_hidden_object_cannot_be_picked() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        scene.objects_mut()[0].visible = false;
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(
            pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
            None
        );
    }

    #[test]
    fn a_flattened_object_cannot_be_picked() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        scene.objects_mut()[0].transform.scale = Vec3::new(1.0, 1.0, 0.0);
        let camera = looking_at(Vec3::ZERO);
        assert_eq!(
            pick(&scene, &camera, VIEWPORT, center(), Shown::default()),
            None
        );
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

    /// Straight down on the plate from 20 mm over the point (0.5, 0.5).
    fn from_above() -> OrbitCamera {
        OrbitCamera {
            target: Vec3::new(0.5, 0.5, 0.0),
            distance_mm: 20.0,
            pitch_rad: std::f32::consts::FRAC_PI_2 - 1e-3,
            ..OrbitCamera::default()
        }
    }

    #[test]
    fn a_face_the_section_cut_away_is_passed_through_to_the_one_under_it() {
        // A cube on the floor and a second one standing over it from 2 to 3 mm.
        let mut scene = Scene::default();
        let floor = add(&mut scene, "floor", Vec3::ZERO);
        add(&mut scene, "over", Vec3::new(0.0, 0.0, 2.0));
        let cut = Shown {
            top_mm: Some(1.5),
            ..Shown::default()
        };

        let (id, hit) = pick_surface(&scene, &from_above(), VIEWPORT, center(), cut)
            .expect("the lower cube's lid is drawn under the cut");
        assert_eq!(id, floor);
        assert!(
            (hit.point.z - 1.0).abs() < 1e-3,
            "the lid of the lower cube is at 1 mm, got {}",
            hit.point
        );
    }

    #[test]
    fn the_cap_across_a_cut_is_material_but_no_surface() {
        let mut scene = Scene::default();
        let cube = add(&mut scene, "cube", Vec3::ZERO);
        let cut = Shown {
            top_mm: Some(0.5),
            ..Shown::default()
        };

        assert!(
            pick_surface(&scene, &from_above(), VIEWPORT, center(), cut).is_none(),
            "the cap has no face of the model to put anything on"
        );
        let (id, hit) = pick_material(&scene, &from_above(), VIEWPORT, center(), cut)
            .expect("the cap is the cube's material");
        assert_eq!(id, cube);
        assert!(
            (hit.point.z - 0.5).abs() < 1e-3 && hit.normal.z > 0.99,
            "the cap lies on the cut and faces up out of what is left, got {} {}",
            hit.point,
            hit.normal
        );
        assert_eq!(
            pick(&scene, &from_above(), VIEWPORT, center(), cut),
            Some(cube),
            "a click on the cap selects the model it cuts"
        );
    }

    #[test]
    fn the_cap_is_met_when_the_face_out_of_the_material_lies_under_the_floor_cut() {
        let mut scene = Scene::default();
        let cube = add(&mut scene, "cube", Vec3::ZERO);
        let cut = Shown {
            floor_mm: Some(0.2),
            top_mm: Some(0.5),
        };
        let (id, hit) = pick_material(&scene, &from_above(), VIEWPORT, center(), cut)
            .expect("the cap at 0.5 mm is drawn over the cube's material");
        assert_eq!(id, cube);
        assert!(
            (hit.point.z - 0.5).abs() < 1e-3,
            "the cap lies on the top cut at 0.5 mm, got {}",
            hit.point
        );
    }

    #[test]
    fn the_cap_hides_a_face_of_another_model_inside_the_material() {
        // The upper cube stands from 1 to 2 mm; the lower one reaches into it up to 1.4 mm.
        let mut scene = Scene::default();
        let upper = add(&mut scene, "upper", Vec3::new(0.0, 0.0, 1.0));
        add(&mut scene, "lower", Vec3::new(0.0, 0.0, 0.4));
        let cut = Shown {
            top_mm: Some(1.8),
            ..Shown::default()
        };
        assert!(
            pick_surface(&scene, &from_above(), VIEWPORT, center(), cut).is_none(),
            "the lower cube's lid at 1.4 mm is under the upper cube's cap"
        );
        assert_eq!(
            pick(&scene, &from_above(), VIEWPORT, center(), cut),
            Some(upper)
        );
    }

    #[test]
    fn nothing_under_the_floor_cut_can_be_pointed_at() {
        let mut scene = Scene::default();
        add(&mut scene, "cube", Vec3::ZERO);
        let cut = Shown {
            floor_mm: Some(2.0),
            ..Shown::default()
        };
        assert_eq!(pick(&scene, &from_above(), VIEWPORT, center(), cut), None);
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

    /// The unit cube hollowed to a cavity spanning 0.2..0.8, as a shell keeps it: the cube
    /// with that box appended wound inward.
    fn hollowed(scene: &mut Scene, id: ObjectId) {
        let mut shell = unit_cube();
        let inner = unit_cube();
        let offset = shell.vertices.len() as u32;
        shell
            .vertices
            .extend(inner.vertices.iter().map(|at| *at * 0.6 + Vec3::splat(0.2)));
        shell.faces.extend(
            inner
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );
        let object = scene.get_mut(id).expect("the cube is on the plate");
        let settings = object
            .hollow
            .asking(&core_volume::HollowSettings::for_wall(0.2));
        object.hollow.take(core_volume::Shell {
            mesh: Arc::new(shell),
            cavity: 12..24,
            cavity_mm3: 0.216,
            voxel_mm: 0.05,
            coarsened: false,
            shape: object.transform.unturned(),
            settings,
        });
        object.keep_support_surface();
    }

    #[test]
    fn a_cut_through_a_cavity_is_seen_into_down_to_its_floor() {
        let mut scene = Scene::default();
        let cube = add(&mut scene, "cube", Vec3::ZERO);
        hollowed(&mut scene, cube);
        let cut = Shown {
            top_mm: Some(0.5),
            ..Shown::default()
        };
        let camera = from_above();
        let down = ray_through(&camera, VIEWPORT, center()).expect("a ray down the middle");
        let (near, wall) = seen_along(&scene, &down, cut).expect("the cut lets the ray in");

        assert!(
            (down.at(wall).z - 0.2).abs() < 1e-3,
            "the cavity's floor at 0.2 mm hides what is under it, not the cut at {}",
            down.at(near).z
        );
    }

    #[test]
    fn a_cut_through_the_wall_is_capped() {
        let mut scene = Scene::default();
        let cube = add(&mut scene, "cube", Vec3::ZERO);
        hollowed(&mut scene, cube);
        let cut = Shown {
            top_mm: Some(0.5),
            ..Shown::default()
        };
        // Down through the wall, a tenth in from the cube's edge.
        let ray = Ray::new(Vec3::new(0.1, 0.5, 20.0), Vec3::NEG_Z);
        let (near, wall) = seen_along(&scene, &ray, cut).expect("the cut lets the ray in");
        assert!(
            (near - wall).abs() < 1e-4,
            "the cap at the cut hides the wall's inside"
        );
    }
}
