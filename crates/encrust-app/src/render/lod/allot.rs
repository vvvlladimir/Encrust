//! Which level each model of a frame is drawn at, so the frame fits what the card can
//! draw and what is given up shows as little as it can.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// One way of drawing a model: the faces it costs, and how far its surface may stand from
/// the model's own on screen, pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub faces: usize,
    pub error_px: f32,
}

/// The levels `allot` chooses among for every model, the model itself first and each
/// lighter one after it.
pub type Candidate = Vec<Step>;

/// The level each of `candidates` is drawn at so the frame draws no more than `budget`
/// faces: one level at a time, always the step whose error shows least, none showing more
/// than `most_px`. What does not fit after that is drawn anyway.
pub fn allot(candidates: &[Candidate], budget: usize, most_px: f32) -> Vec<usize> {
    let mut levels = vec![0; candidates.len()];
    let mut total: usize = candidates
        .iter()
        .filter_map(|levels| levels.first())
        .map(|step| step.faces)
        .sum();
    // Errors are never negative, so their bits order as the numbers do.
    let next = |index: usize, level: usize| {
        candidates[index]
            .get(level + 1)
            .filter(|step| step.error_px <= most_px)
            .map(|step| Reverse((step.error_px.to_bits(), index)))
    };
    let mut steps: BinaryHeap<_> = (0..candidates.len())
        .filter_map(|index| next(index, 0))
        .collect();
    while total > budget {
        let Some(Reverse((_, index))) = steps.pop() else {
            break;
        };
        let level = levels[index];
        let (from, to) = (candidates[index][level], candidates[index][level + 1]);
        total = total - from.faces + to.faces;
        levels[index] = level + 1;
        steps.extend(next(index, level + 1));
    }
    levels
}

/// The levels a resting frame draws: every model one level finer than `drawn` towards
/// `wanted`, so detail comes back over a few frames rather than all in one, and never a
/// level coarser than it already is.
pub fn refine(drawn: &[usize], wanted: &[usize]) -> Vec<usize> {
    drawn
        .iter()
        .zip(wanted)
        .map(|(&drawn, &wanted)| if drawn > wanted { drawn - 1 } else { drawn })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A million-face model whose levels halve it, each showing `px` more error.
    fn model(px: f32) -> Candidate {
        (0..5)
            .map(|level| Step {
                faces: 1_000_000 >> level,
                error_px: px * level as f32,
            })
            .collect()
    }

    #[test]
    fn a_frame_that_fits_is_drawn_whole() {
        let candidates = [model(1.0), model(2.0)];
        assert_eq!(allot(&candidates, 2_000_000, f32::INFINITY), [0, 0]);
    }

    #[test]
    fn the_model_whose_error_shows_least_goes_lighter_first() {
        // The far model's half-face level shows 0.5 px, the near one's 4 px.
        let candidates = [model(4.0), model(0.5)];
        assert_eq!(
            allot(&candidates, 1_500_000, f32::INFINITY),
            [0, 1],
            "half of the far model brings it to 1.5 million"
        );
        assert_eq!(
            allot(&candidates, 1_000_000, f32::INFINITY),
            [1, 4],
            "the far one goes as light as it can, at 2 px, before the near one shows 4"
        );
    }

    #[test]
    fn no_step_shows_more_than_it_may() {
        let candidates = [model(0.3), model(2.0)];
        assert_eq!(
            allot(&candidates, 0, 0.5),
            [1, 0],
            "only the far model's first level stays under half a pixel"
        );
    }

    #[test]
    fn a_model_without_levels_is_drawn_whole() {
        let whole = vec![Step {
            faces: 5_000_000,
            error_px: 0.0,
        }];
        assert_eq!(allot(&[whole], 0, f32::INFINITY), [0]);
    }

    #[test]
    fn detail_comes_back_a_level_a_frame_and_never_goes() {
        assert_eq!(refine(&[4, 2, 0], &[0, 2, 3]), [3, 2, 0]);
    }
}
