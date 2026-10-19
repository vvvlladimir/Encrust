use anyhow::{Result, bail};
use core_engine::project::DrainState;
use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::Trapped;
use core_volume::{HoleSize, HollowMode, VolumeError, plugs, stand_on_end};

use crate::i18n::tr;
use crate::job::{TrapJob, TrapOutcome, TrapRequest, trap_tasks};
use crate::landing::place;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// Space left between a model and the plugs laid out beside it, and between one plug and
/// the next, millimetres.
const PLUG_SPACING_MM: Scalar = 3.0;

/// What a click in the viewport puts down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placing {
    #[default]
    Hole,
    /// Each click adds a point to the channel being laid out on the model it lands on.
    Channel,
}

/// What the Drain tool is set to, and the drainage check it has going.
///
/// The sizes are baked into each hole as it is placed, the way a blocker takes the size it
/// was dropped at, so moving a slider changes the next hole and not the ones standing.
#[derive(Debug)]
pub struct DrainTool {
    /// What the panel is set to, in the shape a project writes it down in (ADR 0191).
    pub state: DrainState,
    pub placing: Placing,
    pub job: Option<TrapJob>,
    /// Whether a check has finished since the plate last changed under it. A check is
    /// asked for by hand and never follows a run; see ADR 0198.
    pub checked: bool,
}

impl Default for DrainTool {
    fn default() -> Self {
        Self {
            // Three millimetres is what a resin slicer offers by default, and drains a
            // cavity of any size the machine can print.
            state: DrainState::round(3.0, 3.0, 1.0),
            placing: Placing::default(),
            job: None,
            checked: false,
        }
    }
}

impl DrainTool {
    /// The hole the plate's numbers drill.
    #[cfg(test)]
    pub fn size(&self) -> HoleSize {
        size_of(&self.state)
    }

    /// The hole drilled into `object`: its own size, or the plate's.
    pub fn size_for(&self, object: &SceneObject) -> HoleSize {
        size_of(self.state_for(object))
    }

    /// Starts looking for resin that cannot get out of anything visible on the plate.
    pub fn start(&mut self, scene: &Scene, layer_height_mm: Scalar) -> Result<()> {
        let tasks = trap_tasks(scene);
        if tasks.is_empty() {
            bail!(tr!("drain-nothing-visible"));
        }

        self.checked = false;
        self.job = Some(TrapJob::spawn(TrapRequest {
            tasks,
            layer_height_mm,
        }));
        Ok(())
    }

    /// Says that the plate has moved under the last check: a cavity was cut, or a hole or
    /// a channel went in or came out, so what the panel shows is no longer current.
    pub fn stale(&mut self) {
        self.checked = false;
    }

    /// Drains a running check into the scene and the stage notice. Returns whether one is
    /// still going, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        match outcome {
            TrapOutcome::Checked(models) => {
                let found: usize = models.iter().map(|model| model.trapped.len()).sum();
                for model in models {
                    if let Some(object) = scene.get_mut(model.id) {
                        object.traps.set(model.trapped);
                    }
                }
                self.checked = true;
                *status = Status::Info(tr!("drain-found", count = found));
            }
            TrapOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// Digs the channel every model has been laying out, and says how many were dug.
    pub fn finish_channels(&mut self, scene: &mut Scene) -> usize {
        self.stale();
        let mut dug = 0;
        for object in scene.targets_mut() {
            let diameter_mm = self.size_for(object).diameter_mm;
            let transform = object.transform;
            if !object.hollow.pending().is_empty()
                && object.hollow.finish_channel(diameter_mm, transform)
            {
                dug += 1;
            }
        }
        dug
    }

    /// Drills a hole into pocket `index` of model `id` alone, at its lowest point, and
    /// takes its mark off. Answers whether a hole went in.
    pub fn drill_pocket(&mut self, scene: &mut Scene, id: ObjectId, index: usize) -> bool {
        let Some(object) = scene.get_mut(id) else {
            return false;
        };
        let Some(target) = object.traps.found().get(index).map(|trapped| trapped.at) else {
            return false;
        };
        self.stale();
        let size = self.size_for(object);
        let drilled =
            object
                .hollow
                .drill_into(&object.mesh, &object.bvh, target, size, object.transform);
        object.traps.remove(index);
        drilled
    }

    /// Drills a hole into every pocket the last check found, and says how many it put in.
    ///
    /// The marks come off with them: whether the holes drained what they were put in is
    /// the next check's answer, not this one's.
    pub fn drill_found(&mut self, scene: &mut Scene) -> usize {
        self.stale();
        let mut drilled = 0;

        for object in scene.targets_mut() {
            let size = self.size_for(object);
            let targets: Vec<_> = object
                .traps
                .found()
                .iter()
                .map(|trapped| trapped.at)
                .collect();
            for target in targets {
                if object.hollow.drill_into(
                    &object.mesh,
                    &object.bvh,
                    target,
                    size,
                    object.transform,
                ) {
                    drilled += 1;
                }
            }
            object.traps.clear();
        }
        drilled
    }

    /// Lays a plug for every hole of every model on the plate beside the model it closes,
    /// and says how many it laid; `drift_mm` is what the resin leaves a fit wider by
    /// (`Compensation::fit_drift_mm`). A model that is hollow and goes through gets plugs as
    /// long as its wall, any other as deep as its hole (ADR 0313).
    pub fn lay_plugs(&self, scene: &mut Scene, drift_mm: Scalar) -> Result<usize, VolumeError> {
        let mut laid = Vec::new();
        for object in scene.targets() {
            let (holes, _) = object.hollow.cut();
            if holes.is_empty() {
                continue;
            }
            let least = object.transform.least_scale();
            let gap_mm = (self.state_for(object).plug_gap_mm - drift_mm) / least;
            let wall_mm = object
                .hollow
                .asked()
                .filter(|asked| asked.mode == HollowMode::Internal)
                .map(|asked| asked.thickness_mm / least);
            let made = plugs(&object.mesh, &object.bvh, holes, wall_mm, gap_mm)?;
            let stood = made.iter().zip(holes).map(|(plug, hole)| {
                let placed = transform_mesh(plug, object.transform);
                stand_on_end(&placed, object.transform.linear() * hole.axis)
            });
            laid.push((object.id, object.name.clone(), stood.collect::<Vec<_>>()));
        }

        let count = laid.iter().map(|(_, _, plugs)| plugs.len()).sum();
        for (id, name, plugs) in laid {
            lay_out(scene, id, &name, plugs);
        }
        Ok(count)
    }

    /// What `object` drills with: its own settings, or the plate's.
    fn state_for<'a>(&'a self, object: &'a SceneObject) -> &'a DrainState {
        object.own.drain.as_ref().unwrap_or(&self.state)
    }
}

/// Stands `plugs` on the plate in a row past the right-hand side of model `beside`.
fn lay_out(scene: &mut Scene, beside: ObjectId, name: &str, plugs: Vec<Mesh>) {
    let Some((bounds, plate)) = scene
        .get(beside)
        .and_then(|object| Some((object.world_bounds()?, object.plate)))
    else {
        return;
    };
    let mut x = bounds.maxs.x + PLUG_SPACING_MM;
    for (index, plug) in plugs.into_iter().enumerate() {
        let Some(size) = plug.aabb().map(|bounds| bounds.extents()) else {
            continue;
        };
        let offset = Vec3::new(x + size.x / 2.0, bounds.mins.y + size.y / 2.0, 0.0);
        let laid = transform_mesh(&plug, Transform::from_translation(offset));
        let name = tr!("drain-plug-name", name = name.to_owned(), index = index + 1);
        place(scene, plate, name, laid);
        x += size.x + PLUG_SPACING_MM;
    }
}

fn size_of(state: &DrainState) -> HoleSize {
    HoleSize {
        diameter_mm: state.diameter_mm,
        depth_mm: state.depth_mm,
        taper: state.taper,
        shape: state.shape,
        turn_deg: state.turn_deg,
        mode: state.mode,
    }
}

/// The pockets of resin the last drainage check found in one model, in its own space.
/// Each carries the box it fills, and the viewport paints the cavity red inside those
/// boxes alone, so draining one pocket takes its own red away (ADR 0200). The check reads
/// the slice stack, which is why these sit beside the model's `ModelHollow` rather than in
/// it; see `docs/decisions/0129`.
#[derive(Debug, Clone, Default)]
pub struct Traps {
    found: Vec<Trapped>,
}

impl Traps {
    pub fn found(&self) -> &[Trapped] {
        &self.found
    }

    pub fn set(&mut self, found: Vec<Trapped>) {
        self.found = found;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Takes the mark off pocket `index` alone.
    pub fn remove(&mut self, index: usize) {
        if index < self.found.len() {
            self.found.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use std::sync::Arc;

    use crate::scene::{ImportSummary, Imported};

    fn scene_with_a_tetrahedron() -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "tetrahedron".to_owned(),
            Arc::new(mesh.clone()),
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
        scene
    }

    #[test]
    fn a_check_runs_only_when_it_is_asked_for() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = DrainTool::default();
        let mut status = Status::default();

        tool.stale();
        assert!(
            tool.job.is_none(),
            "a cut that moved says the last check is out of date and starts no new one"
        );

        tool.start(&scene, 0.05).expect("a closed mesh checks");
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(tool.checked, "the check that was asked for answered");
    }

    #[test]
    fn a_cut_leaves_the_last_check_out_of_date() {
        let mut scene = scene_with_a_tetrahedron();
        let mut tool = DrainTool::default();
        let mut status = Status::default();

        tool.start(&scene, 0.05).expect("a closed mesh checks");
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(tool.checked);

        tool.stale();
        assert!(
            !tool.checked,
            "the plate moved, so what the panel reports is no longer what it holds"
        );
    }

    #[test]
    fn drilling_one_pocket_leaves_the_others_marked() {
        let mut scene = scene_with_a_tetrahedron();
        let id = scene.objects()[0].id;
        let pocket = |x: f32, volume_mm3: f32| Trapped {
            at: Vec3::new(x, 2.0, 2.0),
            volume_mm3,
            bounds: core_geometry::Aabb::new(Vec3::ZERO, Vec3::splat(4.0)),
        };
        let found = vec![pocket(2.0, 10.0), pocket(3.0, 20.0)];
        scene.get_mut(id).expect("just inserted").traps.set(found);

        let mut tool = DrainTool::default();
        tool.drill_pocket(&mut scene, id, 0);
        let left = scene.get(id).expect("still there").traps.found().to_vec();
        assert_eq!(left.len(), 1, "the drilled pocket alone loses its mark");
        assert_eq!(left[0].volume_mm3, 20.0, "the second pocket stays");
        assert!(
            !tool.drill_pocket(&mut scene, id, 5),
            "no pocket five to drill"
        );
    }

    #[test]
    fn a_plug_for_each_hole_stands_on_the_plate_clear_of_its_model() {
        let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "block".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary::made(diagnose(&mesh)),
        ));
        let tool = DrainTool::default();
        let object = scene.get_mut(id).expect("just inserted");
        let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
        for x in [3.0, 7.0] {
            object.hollow.add_drain(
                &mesh,
                &bvh,
                Vec3::new(x, 5.0, 10.0),
                Vec3::Z,
                tool.size(),
                Transform::default(),
            );
        }

        let laid = tool
            .lay_plugs(&mut scene, 0.0)
            .expect("both holes are sound");
        assert_eq!(laid, 2);
        let [block, plugs @ ..] = scene.objects() else {
            panic!("the block and its plugs");
        };
        assert_eq!(plugs.len(), 2, "one plug a hole");
        let edge = block.world_bounds().expect("the block has bounds").maxs.x;
        for plug in plugs {
            let bounds = plug.world_bounds().expect("a plug has bounds");
            assert!(
                bounds.mins.z.abs() < 1e-4,
                "{} stands on the plate",
                plug.name
            );
            assert!(bounds.mins.x > edge, "{} is clear of the block", plug.name);
            assert!(
                (bounds.maxs.z - 3.0).abs() < 1e-3,
                "a solid block's plug is as deep as its 3 mm hole, got {}",
                bounds.maxs.z
            );
        }
    }

    #[test]
    fn an_empty_plate_has_nothing_to_check() {
        let scene = Scene::default();
        let mut tool = DrainTool::default();
        assert!(tool.start(&scene, 0.05).is_err());
    }
}
