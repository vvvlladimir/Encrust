//! What a step does on the plate at once, with no job: holes drilled at points and channels
//! dug, the wall kept solid, supports placed by hand, a model scaled, the supports drilled.

use core_geometry::{Vec2, Vec3};
use core_volume::{DEFAULT_BLOCKER_MM, ModelHollow, hole_at};

use super::Plan;
use super::plan::Drains;
use super::step::Ended;
use crate::i18n::tr;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::Tools;
use crate::status::Status;

/// Drills a hole into the surface nearest each of `drains`' points on each model, at its size
/// or the Drain tool's, as `--drain-at` drills: kept as the model's own size, so its plugs and
/// any search after follow it. Says how many were drilled.
pub(super) fn drill(ids: &[ObjectId], drains: &Drains, scene: &mut Scene, tools: &Tools) -> Status {
    let mut drilled = 0;
    for id in ids {
        let Some(object) = scene.get_mut(*id) else {
            continue;
        };
        if let Some(size) = &drains.size {
            object.own.drain = Some(size.clone());
        }
        let Some(inverse) = object.transform.inverse_matrix() else {
            continue;
        };
        let least = object.transform.least_scale();
        let mut size = tools.drain.size_for(object);
        size.diameter_mm /= least;
        size.depth_mm /= least;
        for at in &drains.at_mm {
            let near = inverse.transform_point3(*at);
            if let Some(hole) = hole_at(&object.mesh, &object.bvh, near, size) {
                object.hollow.add_hole(hole);
                drilled += 1;
            }
        }
        drilled += dig(object, drains, tools);
    }
    Status::Info(tr!("prepare-drilled", count = drilled))
}

/// Scales `object` by `scale` from the way it came in, as `--scale` does before anything else,
/// and stands it back on the middle of the plate.
pub(super) fn scale(object: &mut SceneObject, scale: Vec3, plate: &BuildPlate) {
    object.transform.scale = scale;
    object.slide_onto_plate(Vec2::new(plate.x_mm, plate.y_mm));
    object.stand_on_plate();
}

/// Keeps the wall solid round each of the plan's points on the models it hollows, at the
/// blocker radius of the hollow given or Hollow's default.
pub(super) fn keep_solid(ids: &[ObjectId], plan: &Plan, scene: &mut Scene) {
    let radius_mm = plan
        .hollow_given()
        .map_or(DEFAULT_BLOCKER_MM, |state| state.blocker_mm);
    for id in ids {
        if let Some(object) = scene.get_mut(*id) {
            let transform = object.transform;
            for at in &plan.keep_solid {
                object.hollow.add_blocker(*at, radius_mm, transform);
            }
        }
    }
}

/// Stands a support in `group` under the surface of each model nearest each of `at_mm`, as a
/// click in Supports' Edit mode does. Says how many.
pub(super) fn by_hand_on(
    ids: &[ObjectId],
    at_mm: &[Vec3],
    group: u16,
    scene: &mut Scene,
) -> Status {
    let mut placed = 0;
    for id in ids {
        let Some(object) = scene.get_mut(*id) else {
            continue;
        };
        let transform = object.transform;
        let (Some(inverse), world) = (transform.inverse_matrix(), transform.to_matrix()) else {
            continue;
        };
        for at in at_mm {
            if let Some(found) = object.bvh.closest(inverse.transform_point3(*at)) {
                object
                    .supports
                    .add(world.transform_point3(found.point), transform, group);
                placed += 1;
            }
        }
    }
    Status::Info(tr!("supports-placed", count = placed))
}

/// Digs each of `drains`' channels through the surface nearest its points, as clicks lay one
/// out and Dig digs it. Says how many were dug.
fn dig(object: &mut SceneObject, drains: &Drains, tools: &Tools) -> usize {
    let mut across = tools.drain.size_for(object);
    if let Some(mm) = drains.channel_mm {
        across.diameter_mm = mm;
    }
    let mut dug = 0;
    for channel in &drains.channels {
        for at in channel {
            if let Some(hit) = super::shape::nearest_hit(object, *at) {
                let transform = object.transform;
                object
                    .hollow
                    .add_channel_point(hit.point, hit.normal, transform);
            }
        }
        let transform = object.transform;
        dug += usize::from(object.hollow.finish_channel(across, transform));
    }
    dug
}

/// Drills a hole into the supports of each model nearest each point, as a click on them does,
/// once every model's supports are built; `None` while one is still growing.
pub(super) fn support_holes(
    ids: &[ObjectId],
    at: &[Vec3],
    scene: &mut Scene,
    tools: &Tools,
) -> Option<Ended> {
    let models: Vec<&SceneObject> = scene.named(ids).collect();
    if models
        .iter()
        .any(|object| object.supports_built().is_none())
    {
        return None;
    }
    let mut holes = Vec::new();
    for object in models {
        let size = tools.drain.size_for(object);
        let (Some((groups, bvhs)), transform) = (object.supports_built(), object.transform) else {
            continue;
        };
        for point in at {
            if let Some(hole) = support_hole(groups, bvhs, *point, (size, transform)) {
                holes.push((object.id, hole));
            }
        }
    }
    let drilled = holes.len();
    for (id, hole) in holes {
        if let Some(object) = scene.get_mut(id) {
            object.hollow.add_hole(hole);
        }
    }
    Some(Ended::Landed(Status::Info(tr!(
        "prepare-drilled",
        count = drilled
    ))))
}

/// The hole a click on the support group nearest `point` drills.
fn support_hole(
    groups: &[core_supports::Shaded],
    bvhs: &[std::sync::Arc<core_geometry::Bvh>],
    point: Vec3,
    (size, transform): (core_volume::HoleSize, core_geometry::Transform),
) -> Option<core_volume::DrainHole> {
    let nearest = groups.iter().zip(bvhs).filter_map(|(group, bvh)| {
        let found = bvh.closest(point)?;
        let normal = group
            .mesh
            .triangle(found.face)?
            .normal_unnormalized()
            .try_normalize()?;
        Some((found.point.distance(point), found.point, normal, group, bvh))
    });
    let (_, hit, normal, group, bvh) = nearest.min_by(|a, b| a.0.total_cmp(&b.0))?;
    ModelHollow::support_drain_at(&group.mesh, bvh, hit, normal, size, transform)
}
