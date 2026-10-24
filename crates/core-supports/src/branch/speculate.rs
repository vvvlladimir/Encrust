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
            let live = self.fronts[next.a].alive && self.fronts[next.b].alive;
            if !next.onto_carrier && live && !judged.contains_key(&key(&next)) {
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
