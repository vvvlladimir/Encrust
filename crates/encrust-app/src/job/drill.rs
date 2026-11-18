use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use core_geometry::{Bvh, Mesh, Transform, transform_mesh};
use core_supports::{Shaded, shaded};
use core_volume::{Cancel, Drilled, ModelRead, Progress, drilled_reached};

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model's cuts to take out of it exactly: what it prints from, its cavity's faces once
/// hollowed, and the cuts as `drill` laid them.
pub struct DrillRequest {
    /// The model read once for its drill and its plugs; `source` is it or its shell.
    pub read: Arc<ModelRead>,
    pub source: Arc<Mesh>,
    pub cavity: Option<Range<usize>>,
    pub cuts: Arc<Mesh>,
}

/// The model drilled, or why the engine could not, which leaves its cuts to the winding rule.
pub type DrillOutcome = Result<Drilled, String>;

/// Cuts taken out on their own thread: the engine reads the whole model, a second or so on a
/// dense one (ADR 0322).
pub type DrillJob = Worker<DrillOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_drill(request: DrillRequest) -> DrillJob {
    Worker::spawn(
        // Given up between its steps: a read or a boolean under way ends first.
        move |stop, progress| pooled(|| drill(&request, stop, progress), Err),
        || Err(tr!("job-thread-stopped-drill")),
    )
}

fn drill(request: &DrillRequest, stop: &AtomicBool, progress: &Progress) -> DrillOutcome {
    let asked = || stop.load(Relaxed);
    let cancel = Cancel::when(&asked).reporting(progress);
    let (read, cavity) = (&request.read, request.cavity.clone());
    Drilled::of_read(read, &request.source, cavity, &request.cuts, cancel)
        .map_err(|error| error.to_string())
}

/// One model's raft, in plate coordinates, to print as: its cuts taken out of it.
pub struct PrintedSupportsRequest {
    pub raft: Shaded,
    pub cuts: Option<Arc<Mesh>>,
    /// Where the model stands, which carries its cuts onto the plate.
    pub transform: Transform,
}

/// The raft as it prints, as it was where the cuts miss it, and its hierarchy; `None` where
/// the run was lost.
pub type PrintedSupportsOutcome = Option<(Shaded, Arc<Bvh>)>;

/// A raft drilled and indexed on its own thread: it is a dense mesh.
pub type PrintedSupportsJob = Worker<PrintedSupportsOutcome>;

/// Starts building the printed raft. The thread is detached: it ends on its own.
pub fn spawn_printed_supports(request: PrintedSupportsRequest) -> PrintedSupportsJob {
    Worker::spawn(
        move |_, _| pooled(|| Some(printed_raft(&request)), |_| None),
        || None,
    )
}

fn printed_raft(request: &PrintedSupportsRequest) -> (Shaded, Arc<Bvh>) {
    let drilled = request
        .cuts
        .as_ref()
        .map(|cuts| transform_mesh(cuts, request.transform))
        .and_then(|cuts| drilled_reached(&request.raft.mesh, &cuts));
    let raft = match drilled {
        Some(mesh) => shaded(vec![mesh])
            .pop()
            .unwrap_or_else(|| request.raft.clone()),
        None => request.raft.clone(),
    };
    let bvh = Arc::new(Bvh::build(&raft.mesh));
    (raft, bvh)
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, signed_volume};
    use core_volume::{DrainHole, HoleSize, drill};

    use super::*;

    /// A 4 mm hole down through a 2 mm raft, as a hole drilled from a model above it reaches.
    fn through_the_raft() -> Arc<Mesh> {
        let hole = DrainHole::sized(
            Vec3::new(0.0, 0.0, 4.0),
            Vec3::NEG_Z,
            HoleSize::round(4.0, 8.0, 1.0),
            0.0,
        );
        Arc::new(drill(&[hole], &[]).expect("a sound hole drills"))
    }

    fn raft() -> Shaded {
        let slab = cuboid(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 2.0));
        shaded(vec![slab]).pop().expect("one mesh shades")
    }

    #[test]
    fn a_hole_is_taken_out_of_the_raft_it_reaches() {
        let (drilled, _) = printed_raft(&PrintedSupportsRequest {
            raft: raft(),
            cuts: Some(through_the_raft()),
            transform: Transform::default(),
        });
        // A 24-gon with its corners on 2 mm holds 12 r^2 sin(pi / 12), through 2 mm of raft.
        let bore = 12.0 * 4.0 * (std::f32::consts::PI / 12.0).sin() * 2.0;
        let volume = signed_volume(&drilled.mesh);
        assert!(
            (volume - (800.0 - bore)).abs() < 1e-2,
            "the 800 mm3 raft less the {bore} mm3 bore, got {volume}"
        );
    }

    #[test]
    fn a_raft_the_holes_miss_is_drawn_as_it_stood() {
        let raft = raft();
        let (printed, _) = printed_raft(&PrintedSupportsRequest {
            raft: raft.clone(),
            cuts: Some(through_the_raft()),
            transform: Transform::from_translation(Vec3::new(40.0, 0.0, 0.0)),
        });
        assert!(Arc::ptr_eq(&printed.mesh, &raft.mesh));
    }
}
