//! Which models on the plate, with their supports and raft, stand past the build volume or
//! reach into the plate's inset: what Check lists beside the stripes the view marks.

use std::hash::{DefaultHasher, Hash, Hasher};

use core_geometry::{Aabb, Vec3};

use super::BuildPlate;
use crate::highlight::Spot;
use crate::scene::{ObjectId, Scene, SceneObject};

/// A model on the plate being edited that does not stand clear inside the build volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Astray {
    pub id: ObjectId,
    /// Past the build volume, its overhang cut off the layers; otherwise only in the inset,
    /// which still slices whole.
    pub past: bool,
    /// The part of it that stands out, for the camera to fly to.
    pub spot: Option<Spot>,
}

/// The models on the plate being edited that stand past `plate` or in its inset, kept in
/// `ctx` until a model, its supports or the plate change.
pub fn astray(ctx: &egui::Context, scene: &Scene, plate: &BuildPlate) -> Vec<Astray> {
    let fingerprint = fingerprint(scene, plate);
    let id = egui::Id::new("plate-astray");
    if let Some((cached, astray)) = ctx.data(|data| data.get_temp::<(u64, Vec<Astray>)>(id))
        && cached == fingerprint
    {
        return astray;
    }
    let astray = measured(scene, plate);
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, astray.clone())));
    astray
}

fn measured(scene: &Scene, plate: &BuildPlate) -> Vec<Astray> {
    let (usable_low, usable_high) = plate.usable_area();
    let volume = Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm);
    let usable = (usable_low.extend(0.0), usable_high.extend(plate.z_mm));
    scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            let bounds = bounds(object)?;
            let fit = plate.fit(&bounds);
            let (past, room) = match (fit.fits(), fit.in_inset()) {
                (false, _) => (true, (Vec3::ZERO, volume)),
                (true, true) => (false, usable),
                (true, false) => return None,
            };
            let spot = standing_out(&bounds, room)
                .and_then(|part| Spot::around([part.mins, part.maxs], Vec3::ZERO));
            Some(Astray {
                id: object.id,
                past,
                spot,
            })
        })
        .collect()
}

/// The slab of `bounds` past the box `low`..`high` along the first axis it runs past.
fn standing_out(bounds: &Aabb, (low, high): (Vec3, Vec3)) -> Option<Aabb> {
    (0..3).find_map(|axis| {
        let (mut mins, mut maxs) = (bounds.mins, bounds.maxs);
        if bounds.mins[axis] < low[axis] {
            maxs[axis] = low[axis].min(bounds.maxs[axis]);
        } else if bounds.maxs[axis] > high[axis] {
            mins[axis] = high[axis].max(bounds.mins[axis]);
        } else {
            return None;
        }
        Some(Aabb::new(mins, maxs))
    })
}

/// The box `object` stands in with the supports and raft last drawn under it.
fn bounds(object: &SceneObject) -> Option<Aabb> {
    let supports = object.supports.drawn().iter();
    object
        .world_bounds()
        .into_iter()
        .chain(supports.filter_map(|group| group.mesh.aabb()))
        .reduce(|a, b| Aabb::new(a.mins.min(b.mins), a.maxs.max(b.maxs)))
}

fn fingerprint(scene: &Scene, plate: &BuildPlate) -> u64 {
    let mut hasher = DefaultHasher::new();
    scene.standing_fingerprint().hash(&mut hasher);
    [plate.x_mm, plate.y_mm, plate.z_mm]
        .map(f32::to_bits)
        .hash(&mut hasher);
    let (low, high) = plate.usable_area();
    [low, high]
        .map(|corner| corner.to_array().map(f32::to_bits))
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::fixtures::scene_of_boxes;
    use core_geometry::Vec3;

    /// A 150 × 80 × 165 mm plate with a 5 mm inset along its low X edge.
    fn plate() -> BuildPlate {
        let mut plate = BuildPlate::default();
        plate.inset.min_x_mm = 5.0;
        plate
    }

    #[test]
    fn a_model_past_the_plate_and_one_in_its_inset_are_listed_apart_with_what_stands_out() {
        let scene = scene_of_boxes(&[
            (Vec3::new(20.0, 20.0, 0.0), Vec3::new(30.0, 30.0, 10.0)),
            (Vec3::new(145.0, 20.0, 0.0), Vec3::new(155.0, 30.0, 10.0)),
            (Vec3::new(2.0, 20.0, 0.0), Vec3::new(12.0, 30.0, 10.0)),
        ]);
        let ids: Vec<ObjectId> = scene.objects().iter().map(|object| object.id).collect();
        let astray = measured(&scene, &plate());
        let listed: Vec<(ObjectId, bool)> = astray.iter().map(|one| (one.id, one.past)).collect();
        assert_eq!(
            listed,
            vec![(ids[1], true), (ids[2], false)],
            "x = 145..155 mm runs past the 150 mm plate, x = 2 mm is inside the 5 mm inset"
        );
        let centre = |one: &Astray| one.spot.expect("a part stands out").center;
        assert!(
            (centre(&astray[0]) - Vec3::new(152.5, 25.0, 5.0)).length() < 1e-4,
            "the 5 mm past x = 150 mm, got {}",
            centre(&astray[0])
        );
        assert!(
            (centre(&astray[1]) - Vec3::new(3.5, 25.0, 5.0)).length() < 1e-4,
            "the 3 mm short of x = 5 mm, got {}",
            centre(&astray[1])
        );
    }
}
