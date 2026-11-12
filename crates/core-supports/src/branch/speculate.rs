use super::Forest;
use super::merge::Candidate;
use crate::placed::Placed;
use core_geometry::FastMap;
use rayon::prelude::*;
use std::collections::BinaryHeap;

/// How many merges waiting in the heap are judged at once when one has to be: on the crowded
/// bench 16 beat 8 and 64, between idle cores and merges whose fronts die first.
const AHEAD: usize = 16;

/// Which merges have been judged, by their fronts and drop: a front never moves once made,
/// so the answer holds until the merge is taken or its fronts are gone.
pub(super) type Judged = FastMap<(usize, usize, u32), bool>;

impl Forest<'_> {
    /// Whether `candidate` [`Self::accepts`]: judged with the next merges waiting in `heap`, all
    /// at once, when it has not been yet. The answer is the one judging it alone gives.
    pub(super) fn accepted(
        &self,
        candidate: &Candidate,
        heap: &mut BinaryHeap<Candidate>,
        judged: &mut Judged,
        placed: &Placed,
    ) -> bool {
        if let Some(answer) = judged.get(&key(candidate)) {
            return *answer;
        }
        let mut taken = Vec::with_capacity(AHEAD);
        let mut batch = vec![*candidate];
        while batch.len() < AHEAD
            && let Some(next) = heap.pop()
        {
            // A merge onto a carrier names the carrier as `b`, not a front.
            let live = || self.fronts[next.a].alive && self.fronts[next.b].alive;
            if !next.onto_carrier && live() && !judged.contains_key(&key(&next)) {
                batch.push(next);
            }
            taken.push(next);
        }
        let answers: Vec<bool> = batch
            .par_iter()
            .map(|merge| self.accepts(merge, placed))
            .collect();
        for (merge, answer) in batch.iter().zip(answers) {
            judged.insert(key(merge), answer);
        }
        heap.extend(taken);
        judged[&key(candidate)]
    }
}

fn key(candidate: &Candidate) -> (usize, usize, u32) {
    (candidate.a, candidate.b, candidate.drop_mm.to_bits())
}

#[cfg(test)]
mod tests {
    use crate::SupportPoint;
    use crate::branch::grow;
    use crate::fixtures::profile;
    use crate::group::Profiles;
    use crate::placed::Placed;
    use core_geometry::{Bvh, Mesh, Transform, Vec3};

    #[test]
    fn a_merge_judged_beside_one_onto_a_carrier_reads_no_carrier_as_a_front() {
        // Eight thick supports 40 mm apart, out of each other's reach, are eight carriers;
        // two thin tips beside the last one have two fronts and a merge to judge.
        let mut thin = profile();
        thin.middle.diameter_mm *= 0.5;
        let table = [profile(), thin];
        let mut points: Vec<SupportPoint> = (0..8)
            .map(|at| SupportPoint::new(Vec3::new(40.0 * at as f32, 10.0, 20.0)))
            .collect();
        points.extend(
            [283.0, 285.0].map(|x| SupportPoint::new(Vec3::new(x, 10.0, 20.0)).in_group(1)),
        );
        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let profiles = Profiles::new(&table).expect("two groups");

        let forest = grow(
            &crate::columns(&points, &placed, profiles),
            &placed,
            profiles,
        );

        let held: usize = forest.iter().map(|tree| tree.points().count()).sum();
        assert_eq!(held, 10, "every tip stands");
    }
}
