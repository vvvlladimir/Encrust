use std::sync::Mutex;

use core_geometry::FastMap;

use crate::clear::Beam;
use crate::placed::Placed;

/// How many locks the answers are split over, so the threads of one growth seldom wait on
/// each other; a power of two.
const SHARDS: usize = 16;

/// The most answers one shard keeps before it forgets them all: 16 shards of this hold
/// half a million beams, about 20 MB, more than a dense model's forest asks about.
const MAX_ANSWERS_PER_SHARD: usize = 1 << 15;

/// Whether a beam clears the model, as the clearance test last answered it, kept between
/// growths of one model: grown again after one edit, a forest re-asks only new beams.
#[derive(Debug, Default)]
pub struct Clearances {
    shards: [Mutex<Shard>; SHARDS],
}

#[derive(Debug, Default)]
struct Shard {
    against: Option<Against>,
    answers: FastMap<[u32; 8], bool>,
}

/// What the answers hold for: the surface, by address and size, and the placement on the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Against {
    pub surface: usize,
    pub faces: usize,
    pub matrix: [u32; 16],
}

impl Clearances {
    /// Whether `beam` clears `placed`, from what is kept, or by `sweep` once and kept from then on;
    /// answers kept against another surface or placement are forgotten first.
    pub(crate) fn answer(
        &self,
        placed: &Placed,
        beam: &Beam,
        sweep: impl FnOnce() -> bool,
    ) -> bool {
        let key = key_of(beam);
        let against = placed.against();
        let shard = &self.shards[shard_of(&key)];
        {
            let mut kept = shard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if kept.against != Some(against) {
                kept.answers.clear();
                kept.against = Some(against);
            }
            if let Some(answer) = kept.answers.get(&key) {
                return *answer;
            }
        }
        let answer = sweep();
        let mut kept = shard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if kept.against == Some(against) {
            if kept.answers.len() >= MAX_ANSWERS_PER_SHARD {
                kept.answers.clear();
            }
            kept.answers.insert(key, answer);
        }
        answer
    }
}

fn key_of(beam: &Beam) -> [u32; 8] {
    [
        beam.from.x.to_bits(),
        beam.from.y.to_bits(),
        beam.from.z.to_bits(),
        beam.to.x.to_bits(),
        beam.to.y.to_bits(),
        beam.to.z.to_bits(),
        beam.from_radius_mm.to_bits(),
        beam.to_radius_mm.to_bits(),
    ]
}

/// Which shard `key` is kept in: its words mixed so neighbouring beams spread out.
fn shard_of(key: &[u32; 8]) -> usize {
    let mixed = key.iter().fold(0_u32, |hash, word| {
        (hash.rotate_left(5) ^ word).wrapping_mul(0x9E37_79B9)
    });
    (mixed >> 28) as usize % SHARDS
}

#[cfg(test)]
mod tests;
