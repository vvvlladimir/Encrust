//! A step asked for some of the job's objects stands beside what was asked before for the
//! others; one asked for every object stands in for all of them (ADR 0458).

use super::{Drains, Hollow, Modelling, Recipe, Supports};

/// A step that may name some of the job's objects.
pub trait ForObjects {
    /// The ids it shapes; every object when empty.
    fn objects(&self) -> &[u64];
}

impl ForObjects for Modelling {
    fn objects(&self) -> &[u64] {
        &self.objects
    }
}

impl ForObjects for Hollow {
    fn objects(&self) -> &[u64] {
        &self.objects
    }
}

impl ForObjects for Drains {
    fn objects(&self) -> &[u64] {
        &self.objects
    }
}

impl ForObjects for Supports {
    fn objects(&self) -> &[u64] {
        &self.objects
    }
}

/// Takes `asked` as the latest of a step kept as `first` and `more`: for every object, or none,
/// it stands in for all before it; for some, over what was asked before for those alone.
fn ask<T: ForObjects>(first: &mut Option<T>, more: &mut Vec<T>, asked: Option<T>) {
    match (asked, first.as_ref()) {
        (Some(new), Some(old)) if !new.objects().is_empty() && old.objects() == new.objects() => {
            *first = Some(new);
        }
        (Some(new), Some(_)) if !new.objects().is_empty() => {
            more.retain(|kept| kept.objects() != new.objects());
            more.push(new);
        }
        (asked, _) => {
            *first = asked;
            more.clear();
        }
    }
}

impl Recipe {
    pub fn ask_modelling(&mut self, modelling: Option<Modelling>) {
        ask(&mut self.modelling, &mut self.more_modelling, modelling);
    }

    pub fn ask_hollow(&mut self, hollow: Option<Hollow>) {
        ask(&mut self.hollow, &mut self.more_hollow, hollow);
    }

    pub fn ask_drains(&mut self, drains: Option<Drains>) {
        ask(&mut self.drains, &mut self.more_drains, drains);
    }

    pub fn ask_supports(&mut self, supports: Option<Supports>) {
        ask(&mut self.supports, &mut self.more_supports, supports);
    }

    /// Every modelling asked, the first before those asked later for some objects.
    pub fn modellings(&self) -> impl Iterator<Item = &Modelling> {
        self.modelling.iter().chain(&self.more_modelling)
    }

    /// Every set of drain holes asked, the first before those asked later for some objects.
    pub fn drains_mut(&mut self) -> impl Iterator<Item = &mut Drains> {
        self.drains.iter_mut().chain(&mut self.more_drains)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn plain() -> Recipe {
        Recipe::bare(PathBuf::from("/m/cube.stl"), String::new(), String::new())
    }

    fn hollow_of(id: u64, wall_mm: f32) -> Hollow {
        Hollow {
            wall_mm,
            objects: vec![id],
            ..Default::default()
        }
    }

    #[test]
    fn a_hollow_asked_for_one_piece_keeps_the_other_pieces_hollow() {
        let mut recipe = plain();
        recipe.ask_hollow(Some(hollow_of(1, 2.0)));
        recipe.ask_hollow(Some(hollow_of(2, 3.0)));

        let runs = recipe.runs().expect("the steps run in turn");
        let hollowed: Vec<&Vec<String>> = (runs.iter())
            .filter(|run| run.iter().any(|flag| flag.starts_with("--hollow")))
            .collect();
        assert_eq!(
            hollowed.len(),
            2,
            "piece 1 and piece 2 each hollowed: {runs:?}"
        );
        assert!(hollowed[0].contains(&"--only=1".to_owned()), "{runs:?}");
        assert!(hollowed[1].contains(&"--only=2".to_owned()), "{runs:?}");
    }

    #[test]
    fn a_step_asked_for_every_object_stands_in_for_all_before() {
        let mut recipe = plain();
        recipe.ask_drains(Some(Drains {
            objects: vec![1],
            ..Default::default()
        }));
        recipe.ask_drains(Some(Drains {
            objects: vec![2],
            ..Default::default()
        }));
        recipe.ask_drains(Some(Drains::default()));

        assert!(recipe.more_drains.is_empty());
        assert_eq!(recipe.drains, Some(Drains::default()));
    }

    #[test]
    fn a_cut_of_a_second_piece_keeps_the_cut_of_the_first() {
        let cut = |id| Modelling {
            cut: Some(
                serde_json::from_value(serde_json::json!({"axis": "z", "at_mm": 10.0}))
                    .expect("a cut reads"),
            ),
            objects: vec![id],
            ..Default::default()
        };
        let mut recipe = plain();
        recipe.ask_modelling(Some(cut(1)));
        recipe.ask_modelling(Some(cut(2)));

        assert_eq!(recipe.modellings().count(), 2);
        assert!(recipe.makes_pieces());
    }
}
