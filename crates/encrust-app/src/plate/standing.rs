//! Which models on the plate, with their supports and raft, stand past the build volume or
//! reach into the plate's inset: what the plate card warns of beside the red the view marks.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_geometry::Aabb;

use super::BuildPlate;
use crate::scene::{Scene, SceneObject};

/// How many models on the plate being edited do not stand clear inside the build volume.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Standing {
    /// Models past the build volume, whose overhang is cut off the layers.
    pub off: usize,
    /// Models inside it but reaching into the plate's inset, which still slice whole.
    pub in_inset: usize,
}

/// How the models on the plate being edited stand on `plate`, kept in `ctx` until a model,
/// its supports or the plate change.
pub fn standing(ctx: &egui::Context, scene: &Scene, plate: &BuildPlate) -> Standing {
    let fingerprint = fingerprint(scene, plate);
    let id = egui::Id::new("plate-standing");
    if let Some((cached, standing)) = ctx.data(|data| data.get_temp::<(u64, Standing)>(id))
        && cached == fingerprint
    {
        return standing;
    }
    let standing = measured(scene, plate);
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, standing)));
    standing
}

fn measured(scene: &Scene, plate: &BuildPlate) -> Standing {
    let mut standing = Standing::default();
    let fits = scene
        .printable(scene.active_plate())
        .filter_map(bounds)
        .map(|bounds| plate.fit(&bounds));
    for fit in fits {
        if !fit.fits() {
            standing.off += 1;
        } else if fit.in_inset() {
            standing.in_inset += 1;
        }
    }
    standing
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
    [plate.x_mm, plate.y_mm, plate.z_mm]
        .map(f32::to_bits)
        .hash(&mut hasher);
    plate
        .usable_area()
        .0
        .to_array()
        .map(f32::to_bits)
        .hash(&mut hasher);
    plate
        .usable_area()
        .1
        .to_array()
        .map(f32::to_bits)
        .hash(&mut hasher);
    for object in scene.printable(scene.active_plate()) {
        Arc::as_ptr(&object.mesh).hash(&mut hasher);
        let placed = object.transform.to_matrix().to_cols_array();
        placed.map(f32::to_bits).hash(&mut hasher);
        for group in object.supports.drawn() {
            Arc::as_ptr(&group.mesh).hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::{Transform, Vec3, cuboid, diagnose};

    /// A 150 × 80 × 165 mm plate with a 5 mm inset along its low X edge.
    fn plate() -> BuildPlate {
        let mut plate = BuildPlate::default();
        plate.inset.min_x_mm = 5.0;
        plate
    }

    fn scene_of(boxes: &[(Vec3, Vec3)]) -> Scene {
        let mut scene = Scene::default();
        for (low, high) in boxes {
            let mesh = cuboid(*low, *high);
            let summary = ImportSummary::made(diagnose(&mesh));
            let mesh = Arc::new(mesh);
            scene.insert(Imported::new(
                "box".to_owned(),
                mesh,
                Transform::default(),
                summary,
            ));
        }
        scene
    }

    #[test]
    fn a_model_past_the_plate_and_one_in_its_inset_are_counted_apart() {
        let scene = scene_of(&[
            (Vec3::new(20.0, 20.0, 0.0), Vec3::new(30.0, 30.0, 10.0)),
            (Vec3::new(145.0, 20.0, 0.0), Vec3::new(155.0, 30.0, 10.0)),
            (Vec3::new(2.0, 20.0, 0.0), Vec3::new(12.0, 30.0, 10.0)),
        ]);
        let standing = measured(&scene, &plate());
        assert_eq!(
            standing,
            Standing {
                off: 1,
                in_inset: 1
            },
            "x = 145..155 mm runs past the 150 mm plate, x = 2 mm is inside the 5 mm inset"
        );
    }
}
