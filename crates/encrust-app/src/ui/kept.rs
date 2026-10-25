//! What a panel chose about itself — a section folded, one of a few tabs picked — kept
//! between runs by the preferences file; see `docs/decisions/0334`.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Every choice kept, by the id of the widget that made it.
///
/// An id is a hash of where the widget stands, so one that moved, or a hash that changed
/// with egui, finds nothing and the widget opens on its default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Kept(BTreeMap<u64, serde_json::Value>);

impl Kept {
    /// Everything chosen so far, in this run or an earlier one.
    pub fn of(ctx: &egui::Context) -> Self {
        ctx.data_mut(|data| data.get_temp_mut_or_default::<Self>(store()).clone())
    }

    /// Puts what an earlier run chose back where the widgets look for it.
    pub fn restore(self, ctx: &egui::Context) {
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
        let value = kept.0.get(&id.value())?;
        T::deserialize(value).ok()
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
        kept.0.insert(id.value(), value);
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
    fn a_kept_value_of_another_type_reads_as_nothing_chosen() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("a-tab");
        set(&ctx, id, &"text");
        assert_eq!(get::<bool>(&ctx, id), None);
    }
}
