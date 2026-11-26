//! What a client asks of the plate handed to the open window as edits of it, and an undone cut
//! carried back into the job (ADR 0466, `docs/design/window-link.md`).

use agent_link::ApplyReply;
use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use super::Server;
use super::asked::{Area, Stood, asked_after_check};
use crate::jobs::Revision;
use crate::recipe::{Cut, Recipe};

/// How many times edits are handed again to a window whose plate moved on meanwhile.
pub(super) const REBUILDS: usize = 1;

/// What the client is told when the cut kept was undone in the window.
const CUT_UNDONE: &str = "the cut was undone in the window, so this revision is built on the \
    whole model";

/// What the window says of a revision that asks nothing of its plate.
const NOTHING_ASKED: &str = "the revision asks nothing of the plate the window holds";

/// What the window says of a revision whose cut it already holds.
const CUT_HELD: &str = "the window holds the plate the cut left, and the revision asks nothing \
    more of it";

/// What the client is told when the window's plate moved on each time the edits were handed.
pub(super) const GAVE_UP: &str = "the window's plate changed each time the edits were handed over";

fn cut_of(recipe: &Recipe) -> Option<&Cut> {
    recipe.modelling.as_ref()?.cut.as_ref()
}

impl Server {
    /// Takes the cut out of `recipe` where the window holds `latest`'s cut no longer: the
    /// object it replaced is back on the plate, so the user undid it there.
    pub(super) async fn follow_window_undo(
        &self,
        latest: &Revision,
        recipe: &mut Recipe,
    ) -> Option<&'static str> {
        let object = self.jobs.window_cut(&latest.job, latest.number)?;
        let plate = self.window.plate(&latest.job).await?;
        if !plate.objects.contains(&object) {
            return None;
        }
        recipe.modelling.as_mut()?.cut = None;
        Some(CUT_UNDONE)
    }

    /// Hands what `revision` asks beyond the one before to the open window as edits of the job's
    /// model; `None` for one no edit carries, shown by its check's plate instead (ADR 0473).
    pub(super) async fn edits_in_window(
        &self,
        revision: &Revision,
        report: &Value,
    ) -> Option<Value> {
        let earlier = (revision.number.checked_sub(1))
            .filter(|number| *number > 0)
            .and_then(|number| self.jobs.open(&revision.job, Some(number)).ok())?;
        let held = self.jobs.window_cut(&earlier.job, earlier.number);
        if let Some(held) = held.filter(|_| cut_of(&revision.recipe) == cut_of(&earlier.recipe)) {
            unkept(
                "the window's cut",
                (self.jobs).keep_window_cut(&revision.job, revision.number, held),
            );
            // What is asked of the pieces no edit of the job's model carries: its plate shows it.
            return (revision.recipe == earlier.recipe)
                .then(|| json!({"shown": false, "why_not": CUT_HELD}));
        }
        let earlier_report = self.jobs.report(&earlier).ok();
        let chosen = (
            chosen_turn(report),
            fit_drift_of(report),
            usable_area_of(report),
            stood_of(earlier_report.as_ref()),
        );
        let asked = asked_after_check(&revision.recipe, &earlier.recipe, ObjectId::UNSET, chosen)?;
        if asked.is_empty() {
            return Some(json!({"shown": false, "why_not": NOTHING_ASKED}));
        }
        let edits_on =
            |object| asked_after_check(&revision.recipe, &earlier.recipe, object, chosen);
        self.handed_to_window(revision, edits_on).await
    }
}

impl Server {
    /// Hands the edits `edits_on` builds for the job's object to the open window, built again
    /// on the plate it then holds where it moved on; a cut that lands is noted, so its undo is
    /// followed.
    async fn handed_to_window(
        &self,
        revision: &Revision,
        edits_on: impl Fn(ObjectId) -> Option<Vec<Edit>>,
    ) -> Option<Value> {
        for _ in 0..=REBUILDS {
            let plate = self.window.plate(&revision.job).await?;
            let edits = edits_on(plate.job_object?)?;
            unkept(
                "the edits",
                self.jobs.keep_edits(&revision.job, revision.number, &edits),
            );
            let answer = (self.window)
                .apply(&revision.job, &edits, plate.version)
                .await?;
            if answer.stale {
                continue;
            }
            if let Some(cut) = cut_object(&edits).filter(|_| answer.applied) {
                let kept = self
                    .jobs
                    .keep_window_cut(&revision.job, revision.number, cut);
                unkept("the window's cut", kept);
            }
            return Some(self.answered(revision, &answer));
        }
        Some(self.answered(revision, &ApplyReply::refused(GAVE_UP)))
    }

    /// What the window said of `revision`'s edits; those it did not take are forgotten.
    fn answered(&self, revision: &Revision, answer: &ApplyReply) -> Value {
        if !answer.applied {
            let forgot = self.jobs.forget_edits(&revision.job, revision.number);
            unkept("the edits forgotten", forgot);
        }
        json!(answer)
    }
}

/// The object a cut among `edits` replaces, whose return to the plate is an undo of it.
pub(super) fn cut_object(edits: &[Edit]) -> Option<ObjectId> {
    edits.iter().find_map(|edit| match edit {
        Edit::Cut { object, .. } => Some(*object),
        _ => None,
    })
}

/// Says on stderr what a revision could not keep beside it: the reply stands, but what the
/// window does with it is followed no further.
pub(super) fn unkept(what: &str, kept: anyhow::Result<()>) {
    if let Err(error) = kept {
        eprintln!("encrust-mcp: {what} not kept: {error:#}");
    }
}

/// The fit drift of the check's resin, mm a side.
pub(super) fn fit_drift_of(report: &Value) -> Option<f32> {
    report["slicing"]["fit_drift_mm"]
        .as_f64()
        .map(|drift| drift as f32)
}

/// Whether the job's model stood on supports at the check and whether it was hollowed. A plate
/// whose trees grow again on opening reports none, so a recipe's supports count as well.
pub(super) fn stood_of(report: Option<&Value>) -> Stood {
    let model = report.map_or(&Value::Null, |report| &report["models"][0]);
    let contacts = model["supports"]["contacts"].as_u64().unwrap_or(0);
    let cavity_mm3 = model["hollow"]["cavity_mm3"].as_f64().unwrap_or(0.0);
    (contacts > 0, cavity_mm3 > 0.0)
}

/// The printer's usable area of the check, its low corner and its size, mm.
pub(super) fn usable_area_of(report: &Value) -> Option<Area> {
    let usable = report["models"][0]["fit"]["usable_mm"].as_array()?;
    let pair = |at: usize| {
        let pair = usable.get(at)?.as_array()?;
        let number = |i: usize| pair.get(i)?.as_f64().map(|n| n as f32);
        Some([number(0)?, number(1)?])
    };
    Some((pair(0)?, pair(1)?))
}

/// The turn the check's search chose for the job's model, `--rotate` angles.
fn chosen_turn(report: &Value) -> Option<[f32; 3]> {
    let turn = report["models"][0]["oriented"]["rotate_deg"].as_array()?;
    let angle = |at: usize| turn.get(at)?.as_f64().map(|angle| angle as f32);
    Some([angle(0)?, angle(1)?, angle(2)?])
}
