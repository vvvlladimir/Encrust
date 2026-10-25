use std::ops::Range;
use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Transform, transform_mesh};
use core_supports::{Shaded, shaded};
use core_volume::{Drilled, drilled_reached};
use rayon::prelude::*;

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model's cuts to take out of it exactly: what it prints from, its cavity's faces once
/// hollowed, and the cuts as `drill` laid them.
pub struct DrillRequest {
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
        // The engine cannot be given up on part way.
        move |_, _| pooled(|| drill(&request), Err),
        || Err(tr!("job-thread-stopped-drill")),
    )
}

fn drill(request: &DrillRequest) -> DrillOutcome {
    Drilled::of(&request.source, request.cavity.clone(), &request.cuts)
        .map_err(|error| error.to_string())
}

/// One model's support groups, in plate coordinates, to print as: its cuts taken out of them.
pub struct PrintedSupportsRequest {
    pub groups: Vec<Shaded>,
    pub cuts: Option<Arc<Mesh>>,
    /// Where the model stands, which carries its cuts onto the plate.
    pub transform: Transform,
}

/// The groups as they print, those the cuts miss as they were, and a hierarchy each; `None`
/// where the run was lost.
pub type PrintedSupportsOutcome = Option<(Vec<Shaded>, Vec<Arc<Bvh>>)>;

/// Supports drilled and indexed on their own thread: a raft is a dense mesh.
pub type PrintedSupportsJob = Worker<PrintedSupportsOutcome>;

/// Starts building the printed supports. The thread is detached: it ends on its own.
pub fn spawn_printed_supports(request: PrintedSupportsRequest) -> PrintedSupportsJob {
    Worker::spawn(
        move |_, _| pooled(|| Some(printed_supports(&request)), |_| None),
        || None,
    )
}

fn printed_supports(request: &PrintedSupportsRequest) -> (Vec<Shaded>, Vec<Arc<Bvh>>) {
    let cuts = request
        .cuts
        .as_ref()
        .map(|cuts| transform_mesh(cuts, request.transform));
    let shaded: Vec<Shaded> = request
        .groups
        .par_iter()
        .map(|group| {
            match cuts
                .as_ref()
                .and_then(|cuts| drilled_reached(&group.mesh, cuts))
            {
                Some(mesh) => shaded(vec![mesh]).pop().unwrap_or_else(|| group.clone()),
                None => group.clone(),
            }
        })
        .collect();
    let bvhs = shaded
        .par_iter()
        .map(|group| Arc::new(Bvh::build(&group.mesh)))
        .collect();
    (shaded, bvhs)
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

    #[test]
    fn a_hole_is_taken_out_of_the_raft_it_reaches_and_a_tree_it_misses_is_kept() {
        let raft = cuboid(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 2.0));
        let tree = cuboid(Vec3::new(30.0, 0.0, 0.0), Vec3::new(31.0, 1.0, 10.0));
        let groups = shaded(vec![raft, tree]);
        let (drilled, bvhs) = printed_supports(&PrintedSupportsRequest {
            groups: groups.clone(),
            cuts: Some(through_the_raft()),
            transform: Transform::default(),
        });
        assert_eq!(bvhs.len(), 2, "a hierarchy a group");

        // A 24-gon with its corners on 2 mm holds 12 r^2 sin(pi / 12), through 2 mm of raft.
        let bore = 12.0 * 4.0 * (std::f32::consts::PI / 12.0).sin() * 2.0;
        let volume = signed_volume(&drilled[0].mesh);
        assert!(
            (volume - (800.0 - bore)).abs() < 1e-2,
            "the 800 mm3 raft less the {bore} mm3 bore, got {volume}"
        );
        assert!(
            Arc::ptr_eq(&drilled[1].mesh, &groups[1].mesh),
            "a tree the hole misses is drawn as it stood"
        );
    }
}
