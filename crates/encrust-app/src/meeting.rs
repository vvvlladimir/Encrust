//! Which models on the plate meet another, asked of a thread once the plate settles: the
//! seam they cut traced in red, listed by Check and flown to from it and the models card
//! (ADR 0390).

use std::sync::Arc;

use core_geometry::{Aabb, Transform, Vec3};
use core_plate::BodyKind;

use crate::highlight::{EdgeMark, Highlight, Spot};
use crate::job::{MeetingBody, MeetingJob, spawn_meeting};
use crate::scene::{ObjectId, Scene};
use crate::ui::theme;

/// The models last found to meet, and the search under way.
#[derive(Default)]
pub struct Meeting {
    /// What stood on the plate when `met` was found, as `Scene::standing_fingerprint` reads it.
    found_for: Option<u64>,
    met: Vec<Met>,
    run: Option<Run>,
    /// How many presses have flown to a meeting, so the next goes on to the next place.
    turns: usize,
    flight: Option<Spot>,
}

/// Two models that meet, each place they do and the seam their faces cut, plate mm.
struct Met {
    ids: [ObjectId; 2],
    places: Vec<Aabb>,
    seam: Vec<[Vec3; 2]>,
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
        let job = spawn_meeting(bodies.into());
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
        self.found_for = Some(run.asked_for);
        self.met = met
            .into_iter()
            .map(|meeting| Met {
                ids: meeting.owners.map(|owner| run.owners[owner]),
                places: meeting.places_mm,
                seam: meeting.seam_mm,
            })
            .collect();
        self.run = None;
        false
    }

    /// The meetings found for the plate as it stands, none while it is searched again.
    fn current(&self, scene: &Scene) -> &[Met] {
        match self.found_for == Some(scene.standing_fingerprint()) {
            true => &self.met,
            false => &[],
        }
    }

    /// The pairs of models that meet, lower first, while the answer stands for the plate.
    pub fn pairs(&self, scene: &Scene) -> Vec<[ObjectId; 2]> {
        self.current(scene).iter().map(|met| met.ids).collect()
    }

    /// The models `id` meets, while the answer stands for the plate.
    pub fn met_by(&self, scene: &Scene, id: ObjectId) -> Vec<ObjectId> {
        let other = |met: &Met| match met.ids {
            [one, other] if one == id => Some(other),
            [one, other] if other == id => Some(one),
            _ => None,
        };
        self.current(scene).iter().filter_map(other).collect()
    }

    /// The seam every pair of models cuts, traced in red over them, in every tool.
    pub fn highlight(&self, scene: &Scene) -> Highlight {
        let seam: Vec<[Vec3; 2]> = self
            .current(scene)
            .iter()
            .flat_map(|met| met.seam.iter().copied())
            .collect();
        let edges = (!seam.is_empty()).then(|| EdgeMark {
            edges: seam,
            colour: theme::scene().meets,
        });
        Highlight {
            faces: Vec::new(),
            edges: edges.into_iter().collect(),
        }
    }

    /// Sends the camera to a place where the pair `keep` answers for meet; each press goes
    /// on to the next place.
    pub fn fly_to(&mut self, scene: &Scene, keep: impl Fn([ObjectId; 2]) -> bool) {
        let spots: Vec<Spot> = self
            .current(scene)
            .iter()
            .filter(|met| keep(met.ids))
            .flat_map(|met| &met.places)
            .filter_map(|place| Spot::around([place.mins, place.maxs], Vec3::ZERO))
            .collect();
        if spots.is_empty() {
            return;
        }
        self.flight = Some(spots[self.turns % spots.len()]);
        self.turns += 1;
    }

    /// Where a press asked the camera to go, once.
    pub fn take_flight(&mut self) -> Option<Spot> {
        self.flight.take()
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
