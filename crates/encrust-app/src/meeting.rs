//! Which models on the plate meet another, asked of a thread once the plate settles and
//! named by the plate card and the models card (ADR 0390).

use std::sync::Arc;

use core_geometry::Transform;
use core_plate::BodyKind;

use crate::job::{MeetingBody, MeetingJob, spawn_meeting};
use crate::scene::{ObjectId, Scene};

/// The pairs of models last found to meet, and the search under way.
#[derive(Default)]
pub struct Meeting {
    /// What stood on the plate when `met` was found, as `Scene::standing_fingerprint` reads it.
    found_for: Option<u64>,
    met: Vec<[ObjectId; 2]>,
    run: Option<Run>,
}

struct Run {
    asked_for: u64,
    owners: Vec<ObjectId>,
    job: MeetingJob,
}

impl Meeting {
    /// Starts a search once the plate has settled and differs from what was last searched;
    /// one runs at a time, so a plate moved meanwhile is searched when it ends.
    pub fn start(&mut self, scene: &Scene, settled: bool) {
        if !settled || self.run.is_some() {
            return;
        }
        let asked_for = scene.standing_fingerprint();
        if self.found_for == Some(asked_for) {
            return;
        }
        let (owners, bodies) = bodies_of(scene);
        if owners.len() < 2 {
            self.met.clear();
            self.found_for = Some(asked_for);
            return;
        }
        let job = spawn_meeting(bodies);
        self.run = Some(Run {
            asked_for,
            owners,
            job,
        });
    }

    /// Takes the search's answer on the frame it ends. Returns whether one is still running.
    pub fn poll(&mut self) -> bool {
        let Some(run) = &mut self.run else {
            return false;
        };
        let Some(met) = run.job.poll() else {
            return true;
        };
        let owners = std::mem::take(&mut run.owners);
        self.found_for = Some(run.asked_for);
        self.met = met
            .into_iter()
            .map(|pair| pair.map(|owner| owners[owner]))
            .collect();
        self.run = None;
        false
    }

    /// The pairs that meet, while they were found for the plate as it stands.
    pub fn met(&self, scene: &Scene) -> &[[ObjectId; 2]] {
        match self.found_for == Some(scene.standing_fingerprint()) {
            true => &self.met,
            false => &[],
        }
    }

    /// The models `id` meets, while the answer stands for the plate.
    pub fn met_by(&self, scene: &Scene, id: ObjectId) -> Vec<ObjectId> {
        let other = |&[one, other]: &[ObjectId; 2]| match (one == id, other == id) {
            (true, _) => Some(other),
            (_, true) => Some(one),
            _ => None,
        };
        self.met(scene).iter().filter_map(other).collect()
    }
}

/// Each printable model of the plate being edited, by its place in the answer, and its
/// bodies: itself as it prints, and the supports last drawn under it.
fn bodies_of(scene: &Scene) -> (Vec<ObjectId>, Vec<MeetingBody>) {
    let mut owners = Vec::new();
    let mut bodies = Vec::new();
    for object in scene.printable(scene.active_plate()) {
        let owner = owners.len();
        owners.push(object.id);
        let printed = Arc::clone(object.printed());
        let bvh = Arc::ptr_eq(&printed, &object.mesh).then(|| Arc::clone(&object.bvh));
        bodies.push(MeetingBody {
            owner,
            kind: BodyKind::Model,
            mesh: printed,
            bvh,
            transform: object.transform,
        });
        bodies.extend(object.supports.drawn().iter().map(|group| MeetingBody {
            owner,
            kind: BodyKind::Supports,
            mesh: Arc::clone(&group.mesh),
            bvh: None,
            transform: Transform::default(),
        }));
    }
    (owners, bodies)
}

#[cfg(test)]
mod tests;
