//! The time a run spends in each of its stages, summed by stage, for the report's `stages_s`.
//! Kept per thread: a run's stages are called from the thread that runs it, `rayon` below them.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Instant;

use core_edit::Edit;

thread_local! {
    static SPENT: RefCell<BTreeMap<String, f64>> = const { RefCell::new(BTreeMap::new()) };
}

/// Runs `work`, adding the seconds it took to `stage`.
pub(crate) fn timed<T>(stage: &str, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let done = work();
    let seconds = started.elapsed().as_secs_f64();
    SPENT.with(|spent| *spent.borrow_mut().entry(stage.to_owned()).or_default() += seconds);
    done
}

/// The stage an edit's time is counted under: `edit:` and the word that names its kind.
pub(crate) fn edit_stage(edit: &Edit) -> String {
    let value = serde_json::to_value(edit).unwrap_or_default();
    let kind = value["edit"].as_str().unwrap_or("other");
    format!("edit:{kind}")
}

/// The seconds spent so far by stage, each rounded to the millisecond, those under one left
/// out, the count started anew.
pub(crate) fn take() -> BTreeMap<String, f64> {
    let spent = SPENT.with(|spent| std::mem::take(&mut *spent.borrow_mut()));
    (spent.into_iter())
        .map(|(stage, seconds)| (stage, (seconds * 1000.0).round() / 1000.0))
        .filter(|(_, seconds)| *seconds > 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_edit::ObjectId;

    #[test]
    fn a_stage_timed_twice_sums_and_take_starts_the_count_anew() {
        take();
        timed("import", || {
            std::thread::sleep(std::time::Duration::from_millis(2))
        });
        timed("import", || ());
        timed("check", || ());
        let spent = take();
        assert_eq!(spent.len(), 1, "one stage took a millisecond: {spent:?}");
        assert!(spent["import"] >= 0.002, "two millis at least: {spent:?}");
        assert!(take().is_empty(), "take starts the count anew");
    }

    #[test]
    fn an_edit_is_counted_under_the_word_its_json_names_it_by() {
        let edit = Edit::Delete {
            object: ObjectId(1),
        };
        assert_eq!(edit_stage(&edit), "edit:delete");
    }
}
