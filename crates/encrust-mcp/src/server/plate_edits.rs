//! A cut the client asks for handed to the open window as an edit of its plate, and an undo
//! there carried back into the job (ADR 0466, `docs/design/window-link.md`).

use serde_json::{Value, json};

use super::Server;
use crate::jobs::Revision;
use crate::recipe::{Cut, Recipe};

/// How many times edits are built again on a plate that moved on before the client is told.
const REBUILDS: usize = 1;

/// What the client is told when the cut kept was undone in the window.
const CUT_UNDONE: &str = "the cut was undone in the window, so this revision is built on the \
    whole model";

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

    /// Hands `revision`'s cut to the open window as an edit of the plate it holds, where the
    /// cut is new; `None` leaves the revision to be shown as ADR 0459 runs it.
    pub(super) async fn cut_in_window(&self, revision: &Revision) -> Option<Value> {
        let cut = cut_of(&revision.recipe)?;
        let earlier = (revision.number.checked_sub(1))
            .filter(|number| *number > 0)
            .and_then(|number| self.jobs.open(&revision.job, Some(number)).ok());
        if let Some(earlier) = earlier.filter(|earlier| cut_of(&earlier.recipe) == Some(cut)) {
            let held = self.jobs.window_cut(&earlier.job, earlier.number)?;
            let _ = (self.jobs).keep_window_cut(&revision.job, revision.number, held);
            return Some(json!({"shown": false, "why_not": CUT_HELD}));
        }
        for _ in 0..=REBUILDS {
            let plate = self.window.plate(&revision.job).await?;
            let object = plate.job_object?;
            let edits = [cut.edit(object)];
            let _ = self.jobs.keep_edits(&revision.job, revision.number, &edits);
            let answer = (self.window)
                .apply(&revision.job, &edits, plate.version)
                .await?;
            if answer["stale"] == true {
                continue;
            }
            if answer["applied"] == true {
                let _ = self
                    .jobs
                    .keep_window_cut(&revision.job, revision.number, object);
            }
            return Some(answer);
        }
        Some(json!({"applied": false,
                    "why_not": "the window's plate changed each time the cut was built on it"}))
    }
}
