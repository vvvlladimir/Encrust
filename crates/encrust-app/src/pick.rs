use core_geometry::{Mat4, PlacedHit, Ray, Scalar, Transform, Vec3, raycast_placed};

use crate::camera::OrbitCamera;
use crate::scene::{ObjectId, Scene, SceneObject};

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

/// What a printed surface under the cursor belongs to: its model, or one of the support
/// groups standing under that model, the raft among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Printed {
    Model,
    Supports(usize),
}

/// The printed surface nearest the camera under the cursor, and where the ray met it in plate
/// coordinates: a model as it prints, its holes and cavity included, or its supports as they
/// print once built. Only a face the ray meets from outside counts, none the section cut away.
pub fn pick_printed(
    scene: &Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    shown: Shown,
) -> Option<(ObjectId, Printed, PlacedHit)> {
    let ray = ray_through(camera, viewport, cursor)?;
    let (near, far) = shown.span(&ray)?;
    let from_cut = Ray::new(ray.at(near), ray.direction);
    scene
        .printable(scene.active_plate())
        .flat_map(|object| {
            let (surface, bvh) = object.support_surface();
            let model = raycast_placed(surface, bvh, object.transform, &from_cut)
                .map(|hit| (Printed::Model, hit));
            model
                .into_iter()
                .chain(support_hits(object, from_cut))
                .map(|(printed, hit)| (object.id, printed, hit))
        })
        .map(|(id, printed, hit)| {
            let distance = hit.distance + near;
            (id, printed, PlacedHit { distance, ..hit })
        })
        .filter(|(_, _, hit)| hit.distance <= far)
        .min_by(|(_, _, a), (_, _, b)| a.distance.total_cmp(&b.distance))
        .filter(|(_, _, hit)| hit.normal.dot(ray.direction) < 0.0)
}

/// Where `ray` meets each of `object`'s support groups as they print, once built.
fn support_hits(object: &SceneObject, ray: Ray) -> impl Iterator<Item = (Printed, PlacedHit)> {
    let groups = object
        .supports_built()
        .into_iter()
        .flat_map(|(groups, bvhs)| groups.iter().zip(bvhs).enumerate());
    groups.filter_map(move |(index, (group, bvh))| {
        raycast_placed(&group.mesh, bvh, Transform::default(), &ray)
            .map(|hit| (Printed::Supports(index), hit))
    })
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
mod tests;
