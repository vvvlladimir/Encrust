use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering::Relaxed};

use core_plate::Phase;

use crate::job::Highest;

/// Where the bar stands when the shortlist is cut: weighed on three models, whose search took
/// 3 to 19 % of a run and whose proof took the rest; see ADR 0384.
const LOOKING_END: f32 = 0.12;

/// How far each model of an orientation run has got, written by the threads searching them
/// and read by the window each frame.
#[derive(Debug)]
pub(super) struct Seen {
    models: Vec<Model>,
    shown: Highest,
}

#[derive(Debug, Default)]
struct Model {
    /// The phase in hand plus one, or zero before the model's search has begun.
    phase: AtomicU8,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl Seen {
    pub fn new(models: usize) -> Self {
        Self {
            models: (0..models).map(|_| Model::default()).collect(),
            shown: Highest::default(),
        }
    }

    /// Model `at` is `done` pieces into `total` of `phase`.
    pub fn told(&self, at: usize, phase: Phase, done: usize, total: usize) {
        if let Some(model) = self.models.get(at) {
            model.total.store(total, Relaxed);
            model.done.store(done, Relaxed);
            model.phase.store(phase as u8 + 1, Relaxed);
        }
    }

    /// The share of the run done, the models' shares averaged, or `None` before any model
    /// has begun.
    pub fn share(&self) -> Option<f32> {
        let shares: Vec<Option<f32>> = self.models.iter().map(Model::share).collect();
        shares.iter().any(Option::is_some).then(|| {
            let share = shares.iter().map(|share| share.unwrap_or(0.0)).sum::<f32>();
            self.shown.keep(share / shares.len() as f32)
        })
    }

    /// The phase of the model furthest behind, or `None` before any has begun.
    pub fn phase(&self) -> Option<Phase> {
        self.models
            .iter()
            .filter_map(Model::phase)
            .min_by_key(|phase| *phase as u8)
    }
}

impl Model {
    fn phase(&self) -> Option<Phase> {
        match self.phase.load(Relaxed) {
            1 => Some(Phase::Looking),
            2 => Some(Phase::Proving),
            _ => None,
        }
    }

    fn share(&self) -> Option<f32> {
        let total = self.total.load(Relaxed).max(1);
        let within = self.done.load(Relaxed).min(total) as f32 / total as f32;
        self.phase().map(|phase| match phase {
            Phase::Looking => LOOKING_END * within,
            Phase::Proving => LOOKING_END + (1.0 - LOOKING_END) * within,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_are_averaged_and_the_one_behind_names_the_phase() {
        let seen = Seen::new(2);
        assert_eq!(seen.share(), None, "nothing has begun");

        seen.told(0, Phase::Proving, 3, 3);
        seen.told(1, Phase::Looking, 0, 8);
        let share = seen.share().expect("two models have begun");
        assert!((share - 0.5).abs() < 1e-6, "(1 + 0) / 2, got {share}");
        assert_eq!(seen.phase(), Some(Phase::Looking));

        seen.told(1, Phase::Proving, 3, 3);
        assert_eq!(seen.phase(), Some(Phase::Proving));
    }
}
