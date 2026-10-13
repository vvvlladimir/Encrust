//! Lighter copies of heavy meshes, drawn while the camera moves so a plate of many large
//! models keeps up with the hand; see `docs/design/viewport.md`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Weak};

use core_geometry::{FastMap, Mesh, Scalar, Vec3};

use crate::render::frame_time::CardTime;

/// A mesh with fewer faces than this is drawn whole however the camera moves.
const MIN_FACES: usize = 200_000;
/// The share of its faces a light copy aims to keep, and the fewest it keeps however
/// heavy the mesh: a quarter of a million-face model is still a model.
const KEEP_SHARE: usize = 4;
const KEEP_AT_LEAST: usize = 100_000;
/// How much coarser the lattice gets on each try that keeps too many faces.
const COARSER: Scalar = 1.4;
const TRIES: usize = 4;

/// `mesh` with its corners gathered onto a lattice and each cell's corners merged into
/// their mean, for drawing while the camera moves. `None` when the mesh is light enough
/// to draw whole. Faces whose corners merge go; nothing else about the surface is kept.
pub fn lighter(mesh: &Mesh) -> Option<Mesh> {
    if mesh.faces.len() < MIN_FACES {
        return None;
    }
    let target = (mesh.faces.len() / KEEP_SHARE).max(KEEP_AT_LEAST);
    // A closed surface has about two faces a corner, and a cell of side `c` holds one
    // corner of a surface `c²` large.
    let mut cell = (2.0 * area_mm2(mesh) / target as Scalar).sqrt();
    let mut light = clustered(mesh, cell);
    for _ in 1..TRIES {
        if light.faces.len() <= target + target / 4 {
            break;
        }
        cell *= COARSER;
        light = clustered(mesh, cell);
    }
    Some(light)
}

fn area_mm2(mesh: &Mesh) -> Scalar {
    (0..mesh.faces.len())
        .filter_map(|face| mesh.triangle(face))
        .map(|triangle| triangle.normal_unnormalized().length() / 2.0)
        .sum()
}

fn clustered(mesh: &Mesh, cell: Scalar) -> Mesh {
    let mut cells: FastMap<[i32; 3], u32> = FastMap::default();
    let mut sums: Vec<(Vec3, u32)> = Vec::new();
    let corner_of: Vec<u32> = mesh
        .vertices
        .iter()
        .map(|vertex| {
            let key = (*vertex / cell).floor().as_ivec3().to_array();
            let at = *cells.entry(key).or_insert_with(|| {
                sums.push((Vec3::ZERO, 0));
                (sums.len() - 1) as u32
            });
            let (sum, count) = &mut sums[at as usize];
            *sum += *vertex;
            *count += 1;
            at
        })
        .collect();
    let faces = mesh
        .faces
        .iter()
        .filter(|face| {
            face.iter()
                .all(|&corner| (corner as usize) < corner_of.len())
        })
        .map(|face| face.map(|corner| corner_of[corner as usize]))
        .filter(|[a, b, c]| a != b && b != c && a != c)
        .collect();
    let vertices = sums
        .into_iter()
        .map(|(sum, count)| sum / count as Scalar)
        .collect();
    Mesh::new(vertices, faces)
}

/// The light copies a frame draws in place of their meshes, by the address of the mesh
/// each stands for.
pub type Light = FastMap<usize, Arc<Mesh>>;

/// The frame time the camera is meant to keep up at, seconds: a 60 Hz display.
const FRAME_S: f32 = 1.0 / 60.0;
/// What the card may spend on the scene while the camera moves, seconds: a 60 Hz frame
/// less the window's own pass and presenting it.
const CARD_S: f32 = 0.012;
/// A pass drawing fewer faces than this is timed by its pixels more than by its faces.
const LEARN_FACES: usize = 1_000_000;
/// How much of each new pass the cost of a face takes in: dearer at once, cheaper only
/// slowly, so a card that kept up once does not set every model whole on the next frame.
const DEARER: f32 = 0.5;
const CHEAPER: f32 = 0.05;
/// A frame slower than this while the camera moves counts against the card, seconds: past
/// a display's own interval, and a frame or two of jitter, but not a hand at rest.
const SLOW_S: f32 = 0.022;
const PAUSE_S: f32 = 0.2;
/// Slow frames in a row before the budget is cut, so one hitch does not set it.
const SLOW_RUN: u32 = 3;
/// How far under what kept up the budget is set, so the next frame does not sit on the edge.
const MARGIN: f32 = 0.85;

/// How many faces this machine draws while the camera moves and still keeps up, learnt
/// from the frames themselves: nothing is drawn light until a frame shows it has to be.
///
/// Where the card times its own passes the budget follows what a face costs it, down and
/// back up; elsewhere it is cut by runs of slow frames and never raised.
#[derive(Debug)]
pub struct Budget {
    faces: Option<usize>,
    slow: u32,
    face_s: Option<f32>,
    times: Receiver<CardTime>,
    report: Sender<CardTime>,
}

impl Default for Budget {
    fn default() -> Self {
        let (report, times) = mpsc::channel();
        Self {
            faces: None,
            slow: 0,
            face_s: None,
            times,
            report,
        }
    }
}

impl Budget {
    /// Where the viewport sends what the card took over each frame.
    pub fn report(&self) -> Sender<CardTime> {
        self.report.clone()
    }

    /// Takes in the card's times sent since the last frame.
    pub fn take_times(&mut self) {
        while let Ok(time) = self.times.try_recv() {
            self.timed(time);
        }
    }

    fn timed(&mut self, time: CardTime) {
        if time.faces < LEARN_FACES {
            return;
        }
        let face_s = time.card_s / time.faces as f32;
        let eased = self.face_s.map_or(face_s, |was| {
            let ease = if face_s > was { DEARER } else { CHEAPER };
            was + (face_s - was) * ease
        });
        self.face_s = Some(eased);
        self.faces = Some((CARD_S / eased) as usize);
    }

    /// Takes one frame drawn while the camera moved: `dt_s` after the one before it, which
    /// drew `faces` faces. Where the card is not timed, a run of slow frames sets the budget
    /// to what would have kept up.
    pub fn observe(&mut self, dt_s: f32, faces: usize) {
        if self.face_s.is_some() || dt_s >= PAUSE_S {
            return;
        }
        if dt_s < SLOW_S {
            self.slow = 0;
            return;
        }
        self.slow += 1;
        if self.slow < SLOW_RUN {
            return;
        }
        self.slow = 0;
        let kept_up = (faces as f32 * FRAME_S / dt_s * MARGIN) as usize;
        self.faces = Some(self.faces.map_or(kept_up, |faces| faces.min(kept_up)));
    }

    pub fn faces(&self) -> Option<usize> {
        self.faces
    }
}

/// One mesh the frame draws: how large it looks, as its radius over its distance from the
/// eye, and the faces it costs whole and light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub looks: Scalar,
    pub whole: usize,
    pub light: usize,
}

/// Which of `candidates` are drawn light so the frame fits `budget`: the smallest on
/// screen first, one at a time, so what the eye is on stays whole as long as it can.
pub fn allot(candidates: &[Candidate], budget: usize) -> Vec<bool> {
    let mut light = vec![false; candidates.len()];
    let mut total: usize = candidates.iter().map(|candidate| candidate.whole).sum();
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| candidates[a].looks.total_cmp(&candidates[b].looks));
    for index in order {
        if total <= budget {
            break;
        }
        let candidate = candidates[index];
        if candidate.light < candidate.whole {
            light[index] = true;
            total -= candidate.whole - candidate.light;
        }
    }
    light
}

/// The light copy of every heavy mesh the viewport has drawn, built on a worker the first
/// time a mesh is asked for and let go with it.
pub struct Lods {
    built: FastMap<usize, Built>,
    landed: Receiver<(usize, Option<Arc<Mesh>>)>,
    sender: Sender<(usize, Option<Arc<Mesh>>)>,
}

/// One mesh's entry: the mesh itself, held weakly so the cache never keeps it alive, and
/// its light copy once there is one.
struct Built {
    source: Weak<Mesh>,
    light: Option<Option<Arc<Mesh>>>,
}

impl Default for Lods {
    fn default() -> Self {
        let (sender, landed) = mpsc::channel();
        Self {
            built: FastMap::default(),
            landed,
            sender,
        }
    }
}

impl Lods {
    /// Takes in the copies built since the last frame and drops the entries of meshes
    /// nothing holds any more. Returns whether a copy landed, which is worth a frame.
    pub fn poll(&mut self) -> bool {
        let mut landed = false;
        while let Ok((key, light)) = self.landed.try_recv() {
            if let Some(entry) = self.built.get_mut(&key) {
                entry.light = Some(light);
                landed = true;
            }
        }
        self.built
            .retain(|_, entry| entry.source.strong_count() > 0);
        landed
    }

    /// The light copy of `mesh` if it has one ready. The first time a mesh is asked for,
    /// its copy is started on a worker and the mesh is drawn whole until it lands.
    pub fn of(&mut self, mesh: &Arc<Mesh>) -> Option<Arc<Mesh>> {
        if mesh.faces.len() < MIN_FACES {
            return None;
        }
        let key = Arc::as_ptr(mesh) as usize;
        if let Some(entry) = self
            .built
            .get(&key)
            .filter(|entry| entry.source.ptr_eq(&Arc::downgrade(mesh)))
        {
            return entry.light.clone().flatten();
        }
        self.built.insert(
            key,
            Built {
                source: Arc::downgrade(mesh),
                light: None,
            },
        );
        let (source, sender) = (Arc::clone(mesh), self.sender.clone());
        crate::job::spawn(move || {
            let _ = sender.send((key, lighter(&source).map(Arc::new)));
        });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ball of radius `r` around the origin, `rings` by `segments` facets.
    fn ball(r: Scalar, rings: usize, segments: usize) -> Mesh {
        let at = |ring: usize, segment: usize| {
            let polar = std::f32::consts::PI * ring as Scalar / rings as Scalar;
            let around = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            Vec3::new(
                polar.sin() * around.cos(),
                polar.sin() * around.sin(),
                polar.cos(),
            ) * r
        };
        let mut vertices = vec![at(0, 0)];
        for ring in 1..rings {
            vertices.extend((0..segments).map(|segment| at(ring, segment)));
        }
        vertices.push(at(rings, 0));
        let index =
            |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
        let south = (vertices.len() - 1) as u32;
        let mut faces = Vec::new();
        for segment in 0..segments {
            faces.push([0, index(1, segment), index(1, segment + 1)]);
            faces.push([
                index(rings - 1, segment),
                south,
                index(rings - 1, segment + 1),
            ]);
        }
        for ring in 1..rings - 1 {
            for segment in 0..segments {
                faces.push([
                    index(ring, segment),
                    index(ring + 1, segment),
                    index(ring + 1, segment + 1),
                ]);
                faces.push([
                    index(ring, segment),
                    index(ring + 1, segment + 1),
                    index(ring, segment + 1),
                ]);
            }
        }
        Mesh::new(vertices, faces)
    }

    #[test]
    fn nothing_is_budgeted_while_frames_keep_up() {
        let mut budget = Budget::default();
        for _ in 0..100 {
            budget.observe(FRAME_S, 18_000_000);
        }
        assert_eq!(budget.faces(), None);
    }

    #[test]
    fn a_run_of_slow_frames_sets_what_would_have_kept_up() {
        let mut budget = Budget::default();
        budget.observe(1.0 / 30.0, 18_000_000);
        budget.observe(1.0 / 30.0, 18_000_000);
        assert_eq!(budget.faces(), None, "two slow frames are a hitch");
        budget.observe(1.0 / 30.0, 18_000_000);
        let faces = budget.faces().expect("three in a row are the card");
        assert!(
            (7_000_000..9_000_000).contains(&faces),
            "half the faces kept up at twice the rate, less the margin: {faces}"
        );
    }

    #[test]
    fn a_hand_at_rest_is_not_a_slow_frame() {
        let mut budget = Budget::default();
        for _ in 0..10 {
            budget.observe(0.5, 18_000_000);
        }
        assert_eq!(budget.faces(), None);
    }

    fn timed(card_s: f32, faces: usize) -> CardTime {
        CardTime { card_s, faces }
    }

    #[test]
    fn a_card_that_keeps_up_draws_everything_whole() {
        let mut budget = Budget::default();
        budget.timed(timed(0.006, 18_000_000));
        let faces = budget.faces().expect("a timed pass sets the budget");
        assert!(
            faces > 18_000_000,
            "6 ms for 18 million faces leaves room under 12 ms: {faces}"
        );
    }

    #[test]
    fn a_timed_budget_falls_at_once_and_comes_back_slowly() {
        let mut budget = Budget::default();
        budget.timed(timed(0.006, 18_000_000));
        let calm = budget.faces().expect("timed");
        budget.timed(timed(0.060, 18_000_000));
        let busy = budget.faces().expect("timed");
        assert!(
            busy < calm / 4,
            "half way to ten times dearer: {busy} of {calm}"
        );
        budget.timed(timed(0.006, 18_000_000));
        let after = budget.faces().expect("timed");
        assert!(
            busy < after && after < calm / 2,
            "one cheap pass raises it a little: {busy} < {after} < {calm}"
        );
        for _ in 0..200 {
            budget.timed(timed(0.006, 18_000_000));
        }
        let back = budget.faces().expect("timed");
        assert!(
            back * 100 > calm * 99,
            "and a run of them all the way: {back} of {calm}"
        );
    }

    #[test]
    fn a_light_pass_teaches_the_budget_nothing() {
        let mut budget = Budget::default();
        budget.timed(timed(0.010, 500_000));
        assert_eq!(
            budget.faces(),
            None,
            "half a million faces cost their pixels"
        );
    }

    #[test]
    fn a_timed_card_is_not_judged_by_its_frame_intervals() {
        let mut budget = Budget::default();
        budget.timed(timed(0.006, 18_000_000));
        let timed_faces = budget.faces();
        for _ in 0..10 {
            budget.observe(1.0 / 20.0, 18_000_000);
        }
        assert_eq!(
            budget.faces(),
            timed_faces,
            "a slow interval is the window's"
        );
    }

    #[test]
    fn times_sent_to_the_budget_land_on_the_next_frame() {
        let mut budget = Budget::default();
        budget
            .report()
            .send(timed(0.024, 12_000_000))
            .expect("the budget holds its own receiver");
        assert_eq!(budget.faces(), None, "nothing is taken in until asked");
        budget.take_times();
        let faces = budget.faces().expect("taken in");
        assert!(
            (5_900_000..6_100_000).contains(&faces),
            "2 ns a face makes 12 ms six million faces: {faces}"
        );
    }

    #[test]
    fn the_smallest_on_screen_go_light_first_and_only_as_many_as_needed() {
        let candidate = |looks| Candidate {
            looks,
            whole: 1_000_000,
            light: 100_000,
        };
        let candidates = [candidate(0.5), candidate(0.1), candidate(0.3)];
        assert_eq!(allot(&candidates, 3_000_000), [false; 3], "it all fits");
        assert_eq!(
            allot(&candidates, 2_200_000),
            [false, true, false],
            "the smallest alone brings it to 2.1 million"
        );
        assert_eq!(allot(&candidates, 1_000_000), [true, true, true]);
    }

    #[test]
    fn a_light_mesh_is_drawn_whole() {
        assert!(lighter(&ball(10.0, 40, 40)).is_none());
    }

    #[test]
    fn a_heavy_ball_keeps_the_faces_it_aims_for_and_its_shape() {
        let heavy = ball(20.0, 500, 500);
        let light = lighter(&heavy).expect("half a million faces is heavy");
        let target = (heavy.faces.len() / KEEP_SHARE).max(KEEP_AT_LEAST);
        assert!(
            light.faces.len() <= target + target / 4 && light.faces.len() > target / 4,
            "{} faces of {} for a target of {target}",
            light.faces.len(),
            heavy.faces.len()
        );
        let off = light
            .vertices
            .iter()
            .map(|vertex| (vertex.length() - 20.0).abs())
            .fold(0.0, Scalar::max);
        assert!(
            off < 0.5,
            "a merged corner stays near the 20 mm sphere, {off} mm off"
        );
    }
}
