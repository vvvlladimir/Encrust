//! A revision made of edits, checked on the open window's plate with what the user did there,
//! and kept as that plate and its edits (ADR 0456, `docs/design/window-link.md`).

use std::path::PathBuf;

use agent_link::ApplyReply;
use anyhow::Result;
use core_edit::{Edit, ObjectId};
use serde_json::json;

use crate::runner::Outcome;

use super::Server;
use super::asked::{Area, Checked, Stood, asked_after_check, asked_of_pieces};
use super::plate_edits::{
    GAVE_UP, REBUILDS, cut_object, fit_drift_of, pieces_of, stood_of, unkept, usable_area_of,
};
use super::reply::{Reply, not_applied, not_taken, revised};
use crate::jobs::Revision;
use crate::recipe::Recipe;
use crate::server::Context;

/// The plate a revision's edits are made on.
struct Base {
    path: PathBuf,
    /// The window's version of it, where the window handed it over.
    version: Option<u64>,
    /// The job's model on it.
    object: ObjectId,
}

/// What a revision asks: the one before it, its recipe, and the note for the client.
pub(super) type Asked<'a> = (&'a Revision, Recipe, Option<&'a str>);

impl Server {
    /// Checks and keeps `asked` as edits on a plate where every change it makes is an edit and
    /// there is a plate to make them on; otherwise hands its recipe back, for the flags.
    pub(super) async fn edit_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        against: &[String],
        ctx: &Context,
    ) -> Result<Result<Reply>, Box<Recipe>> {
        if let Some(on) = self.pieces_on_plate(latest, &recipe) {
            let job = latest.job.clone();
            let reply = (self.checked_on_plate((latest, recipe, note), on, against, ctx)).await;
            return Ok(self.shown_kept(&job, reply).await);
        }
        let (drift, area, stood) = self.earlier_check(latest, &recipe);
        let checked = (None, drift, area, stood);
        let asked = asked_after_check(&recipe, &latest.recipe, ObjectId::UNSET, checked);
        if asked.as_ref().is_none_or(Vec::is_empty) {
            return Err(Box::new(recipe));
        }
        let Some(base) = self.base_of(latest).await else {
            return Err(Box::new(recipe));
        };
        let Some(edits) = asked_after_check(&recipe, &latest.recipe, base.object, checked) else {
            return Err(Box::new(recipe));
        };
        let on = (base, edits, checked);
        Ok(self
            .checked_on_plate((latest, recipe, note), on, against, ctx)
            .await)
    }

    /// The plate `latest` left in pieces and what `recipe` asks of each as edits; `None` where
    /// it is no such plate or one change is no edit, for the flags.
    fn pieces_on_plate(
        &self,
        latest: &Revision,
        recipe: &Recipe,
    ) -> Option<(Base, Vec<Edit>, Checked)> {
        let path = self.jobs.kept_pieces(latest)?;
        let report = self.jobs.report(latest).ok()?;
        let (drift, area, stood) = self.earlier_check(latest, recipe);
        let edits = asked_of_pieces(recipe, &latest.recipe, &pieces_of(&report), (drift, area))?;
        let base = Base {
            path,
            version: None,
            object: ObjectId::UNSET,
        };
        (!edits.is_empty()).then_some((base, edits, (None, drift, area, stood)))
    }

    /// Shows the open window the revision `reply` kept, as a check by flags is shown.
    async fn shown_kept(&self, job: &str, mut reply: Result<Reply>) -> Result<Reply> {
        if let Ok(kept) = &mut reply
            && !kept.failed
            && let Ok(revision) = self.jobs.open(job, None)
            && let Some(plate) = self.jobs.made_on_plate(&revision)
            && let Some(shown) = self.window.show(&revision, &plate, false).await
        {
            kept.body["window"] = shown;
        }
        reply
    }

    /// The fit drift and usable plate area the previous revision's check reported, where it ran
    /// on the printer and resin `recipe` names, and what the model stood on there: this
    /// revision's check has not run yet.
    fn earlier_check(
        &self,
        latest: &Revision,
        recipe: &Recipe,
    ) -> (Option<f32>, Option<Area>, Stood) {
        let same = latest.recipe.printer == recipe.printer && latest.recipe.resin == recipe.resin;
        let report = self.jobs.report(latest).ok();
        let stood = stood_of(report.as_ref());
        let Some(report) = report.filter(|_| same) else {
            return (None, None, stood);
        };
        (fit_drift_of(&report), usable_area_of(&report), stood)
    }

    /// The open window's plate with the job's model on it, which holds whatever the user did
    /// there; with no window answering, the plate `latest` kept.
    async fn base_of(&self, latest: &Revision) -> Option<Base> {
        if let Some(plate) = self.window.plate(&latest.job).await {
            return Some(Base {
                object: plate.job_object?,
                path: plate.path,
                version: Some(plate.version),
            });
        }
        let (path, object) = self.jobs.kept_base(latest)?;
        Some(Base {
            path,
            version: None,
            object,
        })
    }

    /// Checks `edits` on `base` and hands them to the window that handed it over; where its plate
    /// moved on meanwhile, they are built and checked again on the plate it then holds.
    async fn checked_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        (mut base, mut edits, checked): (Base, Vec<Edit>, Checked),
        against: &[String],
        ctx: &Context,
    ) -> Result<Reply> {
        let mut rebuilt = 0;
        loop {
            let (outcome, written) = self
                .check_edits(&base, &edits, &recipe, against, ctx)
                .await?;
            if !outcome.reported() {
                let _ = std::fs::remove_file(&written);
                return Ok(not_applied(Some(latest), &outcome));
            }
            let answer = match base.version {
                Some(version) => Some(self.window.apply(&latest.job, &edits, version).await),
                None => None,
            };
            let stale = answer
                .as_ref()
                .is_some_and(|a| a.as_ref().is_some_and(|a| a.stale));
            if stale && rebuilt < REBUILDS {
                rebuilt += 1;
                if let Some(next) = self.rebased(latest, &recipe, checked).await {
                    let _ = std::fs::remove_file(&written);
                    (base, edits) = next;
                    continue;
                }
            }
            // A window that answered no holds its plate as before; one gone holds nothing.
            let refused = answer
                .as_ref()
                .is_some_and(|a| a.as_ref().is_some_and(|a| !a.applied));
            let window = answer.map(|answer| match answer {
                Some(_) if stale => ApplyReply::refused(GAVE_UP),
                Some(answer) => answer,
                None => ApplyReply::refused("the window did not answer"),
            });
            if refused {
                let _ = std::fs::remove_file(&written);
                return Ok(not_taken(latest, &json!(window)));
            }
            let kept = (edits.as_slice(), written.as_path(), window);
            let reply = self.keep_on_plate((latest, recipe, note), kept, &outcome);
            // Kept, the plate was moved into the job; anything left here was not.
            let _ = std::fs::remove_file(&written);
            return reply;
        }
    }

    /// The window's plate as it now stands and `recipe`'s edits built again on it.
    async fn rebased(
        &self,
        latest: &Revision,
        recipe: &Recipe,
        checked: Checked,
    ) -> Option<(Base, Vec<Edit>)> {
        let plate = self.window.plate(&latest.job).await?;
        let object = plate.job_object?;
        let edits = asked_after_check(recipe, &latest.recipe, object, checked)?;
        let base = Base {
            path: plate.path,
            version: Some(plate.version),
            object,
        };
        Some((base, edits))
    }

    /// A check of `edits` on `base`, and the plate it leaves.
    async fn check_edits(
        &self,
        base: &Base,
        edits: &[Edit],
        recipe: &Recipe,
        against: &[String],
        ctx: &Context,
    ) -> Result<(Outcome, PathBuf)> {
        let written = self.jobs.scratch_plate()?;
        let asked = written.with_extension("edits.json");
        std::fs::write(&asked, serde_json::to_string(edits)?)?;
        let mut args = vec!["estimate".to_owned(), base.path.display().to_string()];
        args.extend(recipe.machine_flags());
        args.extend(against.iter().cloned());
        args.push(format!("--edits={}", asked.display()));
        args.push(format!("--project-out={}", written.display()));
        let outcome = self.watched(&args, ctx).await;
        let _ = std::fs::remove_file(&asked);
        if outcome.is_err() {
            let _ = std::fs::remove_file(&written);
        }
        Ok((outcome?, written))
    }

    /// Keeps the revision checked as its plate and edits, with what the window said of them.
    fn keep_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        (edits, written, window): (&[Edit], &std::path::Path, Option<ApplyReply>),
        outcome: &Outcome,
    ) -> Result<Reply> {
        let revision = self
            .jobs
            .commit(&latest.job, (recipe, None), &outcome.report)?;
        let (job, number) = (&revision.job, revision.number);
        unkept("the edits", self.jobs.keep_edits(job, number, edits));
        unkept("the plate", self.jobs.keep_plate(job, number, written));
        let landed = window.as_ref().is_some_and(|answer| answer.applied);
        if let Some(cut) = cut_object(edits).filter(|_| landed) {
            unkept(
                "the window's cut",
                self.jobs.keep_window_cut(job, number, cut),
            );
        }
        let mut reply = revised(&revision, outcome, note);
        if let Some(window) = window {
            reply.body["window"] = json!(window);
        }
        Ok(reply)
    }
}
