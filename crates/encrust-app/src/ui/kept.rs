//! What a panel chose about itself — a section folded, one of a few tabs picked — kept
//! between runs by the preferences file; see `docs/decisions/0334` and `0345`.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// How many runs a choice no widget read or made is kept for before it is forgotten.
const KEEP_RUNS: u64 = 20;

/// Every choice kept, by the id of the widget that made it, and the run it was last used in.
///
/// An id is a hash of where the widget stands, so one that moved, or a hash that changed
/// with egui, finds nothing; what it left is forgotten after [`KEEP_RUNS`] (ADR 0345).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "Stored")]
pub struct Kept {
    run: u64,
    choices: BTreeMap<u64, Choice>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Choice {
    value: serde_json::Value,
    used: u64,
}

/// The file as this version writes it, or as one before runs were counted wrote it: a bare
/// map, every entry of which counts as used in run 0. Ids are read as text, as an untagged
/// enum cannot read a number out of a JSON key.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Counted {
        run: u64,
        choices: BTreeMap<String, Choice>,
    },
    Bare(BTreeMap<String, serde_json::Value>),
}

impl From<Stored> for Kept {
    fn from(stored: Stored) -> Self {
        let (run, choices) = match stored {
            Stored::Counted { run, choices } => (run, choices),
            Stored::Bare(values) => (
                0,
                values
                    .into_iter()
                    .map(|(id, value)| (id, Choice { value, used: 0 }))
                    .collect(),
            ),
        };
        let choices = choices
            .into_iter()
            .filter_map(|(id, choice)| Some((id.parse().ok()?, choice)))
            .collect();
        Self { run, choices }
    }
}

impl Kept {
    /// Everything chosen and still in use, in this run or one of the last [`KEEP_RUNS`].
    pub fn of(ctx: &egui::Context) -> Self {
        let mut kept = ctx.data_mut(|data| data.get_temp_mut_or_default::<Self>(store()).clone());
        let run = kept.run;
        kept.choices
            .retain(|_, choice| run.saturating_sub(choice.used) < KEEP_RUNS);
        kept
    }

    /// Puts what an earlier run chose back where the widgets look for it, as a run of its own.
    pub fn restore(mut self, ctx: &egui::Context) {
        self.run += 1;
        ctx.data_mut(|data| data.insert_temp(store(), self));
    }
}

fn store() -> egui::Id {
    egui::Id::new("kept-choices")
}

/// What the widget at `id` last chose, or `None` when it never chose or what it kept no
/// longer reads as a `T`.
pub fn get<T: DeserializeOwned>(ctx: &egui::Context, id: egui::Id) -> Option<T> {
    ctx.data_mut(|data| {
        let kept = data.get_temp_mut_or_default::<Kept>(store());
        let run = kept.run;
        let choice = kept.choices.get_mut(&id.value())?;
        choice.used = run;
        T::deserialize(&choice.value).ok()
    })
}

/// Keeps `value` as what the widget at `id` chose.
pub fn set<T: Serialize>(ctx: &egui::Context, id: egui::Id, value: &T) {
    // Every value kept here is a flag or a plain enum, which always serialises.
    let Ok(value) = serde_json::to_value(value) else {
        return;
    };
    ctx.data_mut(|data| {
        let kept = data.get_temp_mut_or_default::<Kept>(store());
        let used = kept.run;
        kept.choices.insert(id.value(), Choice { value, used });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_survives_the_trip_through_the_preferences_file() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("a-section").with("open");
        set(&ctx, id, &false);

        let text = serde_json::to_string(&Kept::of(&ctx)).expect("the choices serialise");
        let next_run = egui::Context::default();
        serde_json::from_str::<Kept>(&text)
            .expect("and read back")
            .restore(&next_run);
        assert_eq!(get::<bool>(&next_run, id), Some(false));
    }

    #[test]
    fn a_file_from_before_runs_were_counted_still_reads() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("a-section").with("open");
        let text = format!("{{\"{}\": false}}", id.value());
        serde_json::from_str::<Kept>(&text)
            .expect("the bare map reads")
            .restore(&ctx);
        assert_eq!(get::<bool>(&ctx, id), Some(false));
    }

    #[test]
    fn a_choice_no_widget_uses_for_long_enough_is_forgotten() {
        let (used, left) = (egui::Id::new("used"), egui::Id::new("left behind"));
        let mut ctx = egui::Context::default();
        set(&ctx, used, &true);
        set(&ctx, left, &true);
        for _ in 0..KEEP_RUNS {
            let next_run = egui::Context::default();
            Kept::of(&ctx).restore(&next_run);
            ctx = next_run;
            assert_eq!(
                get::<bool>(&ctx, used),
                Some(true),
                "a widget still reads it"
            );
        }
        let next_run = egui::Context::default();
        Kept::of(&ctx).restore(&next_run);
        assert_eq!(get::<bool>(&next_run, used), Some(true));
        assert_eq!(
            get::<bool>(&next_run, left),
            None,
            "unread for {KEEP_RUNS} runs, it is gone"
        );
    }

    #[test]
    fn a_kept_value_of_another_type_reads_as_nothing_chosen() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("a-tab");
        set(&ctx, id, &"text");
        assert_eq!(get::<bool>(&ctx, id), None);
    }
}
