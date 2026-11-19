//! What a client asks of the plate handed to the open window as edits of it, and an undone cut
//! carried back into the job (ADR 0466, `docs/design/window-link.md`).

use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use super::Server;
use super::asked::asked_after_check;
use crate::jobs::Revision;
use crate::recipe::{Cut, Recipe};

/// How many times edits are built again on a plate that moved on before the client is told.
const REBUILDS: usize = 1;

/// What the client is told when the cut kept was undone in the window.
const CUT_UNDONE: &str = "the cut was undone in the window, so this revision is built on the \
    whole model";

/// What the window says of a revision that asks nothing of its plate.
const NOTHING_ASKED: &str = "the revision asks nothing of the plate the window holds";

/// What the window says of a revision whose cut it already holds.
const CUT_HELD: &str = "the window holds the plate the cut left; the other steps of a revision \
    reach it as edits once each is one (ADR 0466)";

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

    /// Hands what `revision` asks of the plate beyond the revision before to the open window
    /// as edits of the job's model there; `None` leaves the revision to be shown as ADR 0459
    /// runs it: the first, one the window has not read the model for, or one asking what no
    /// edit carries yet.
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
            let _ = (self.jobs).keep_window_cut(&revision.job, revision.number, held);
            return Some(json!({"shown": false, "why_not": CUT_HELD}));
        }
        let chosen = chosen_turn(report);
        let asked = asked_after_check(&revision.recipe, &earlier.recipe, ObjectId::UNSET, chosen)?;
        if asked.is_empty() {
            return Some(json!({"shown": false, "why_not": NOTHING_ASKED}));
        }
        for _ in 0..=REBUILDS {
            let plate = self.window.plate(&revision.job).await?;
            let object = plate.job_object?;
            let edits = asked_after_check(&revision.recipe, &earlier.recipe, object, chosen)?;
            let _ = self.jobs.keep_edits(&revision.job, revision.number, &edits);
            let answer = (self.window)
                .apply(&revision.job, &edits, plate.version)
                .await?;
            if answer["stale"] == true {
                continue;
            }
            let cut = edits.iter().any(|edit| matches!(edit, Edit::Cut { .. }));
            if cut && answer["applied"] == true {
                let _ = self
                    .jobs
                    .keep_window_cut(&revision.job, revision.number, object);
            }
            return Some(answer);
        }
        Some(json!({"applied": false,
                    "why_not": "the window's plate changed each time the edits were built on it"}))
    }
}

/// The turn the check's search chose for the job's model, `--rotate` angles.
fn chosen_turn(report: &Value) -> Option<[f32; 3]> {
    let turn = report["models"][0]["oriented"]["rotate_deg"].as_array()?;
    let angle = |at: usize| turn.get(at)?.as_f64().map(|angle| angle as f32);
    Some([angle(0)?, angle(1)?, angle(2)?])
}
