//! A revision made of edits, checked on the open window's plate with what the user did there,
//! and kept as that plate and its edits (ADR 0456, `docs/design/window-link.md`).

use std::path::PathBuf;

use anyhow::Result;
use core_edit::{Edit, ObjectId};

use crate::runner::Outcome;

use super::Server;
use super::asked::{Area, Stood, asked_after_check};
use super::plate_edits::{fit_drift_of, pending, stood_of, unkept, usable_area_of};
use super::points::on_plate;
use super::reply::{Reply, not_applied, revised};
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
        // A plate of pieces is no one model for edits of it to name; a window still taking what
        // was handed before is not waited for (ADR 0467).
        if latest.recipe.makes_pieces() || self.window.aside().busy() {
            return Err(Box::new(recipe));
        }
        let (drift, area, stood) = self.earlier_check(latest, &recipe);
        let checked = (None, drift, area, stood);
        let report = self.jobs.report(latest).unwrap_or_default();
        let (now, then) = (
            on_plate(&recipe, &report),
            on_plate(&latest.recipe, &report),
        );
        let asked = asked_after_check(&now, &then, ObjectId::UNSET, checked);
        if asked.as_ref().is_none_or(Vec::is_empty) {
            return Err(Box::new(recipe));
        }
        let Some(base) = self.base_of(latest).await else {
            return Err(Box::new(recipe));
        };
        let Some(edits) = asked_after_check(&now, &then, base.object, checked) else {
            return Err(Box::new(recipe));
        };
        let on = (base, edits);
        Ok(self
            .checked_on_plate((latest, recipe, note), on, against, ctx)
            .await)
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
    /// there; none without a window answering, where the flags stage the revision (ADR 0467).
    async fn base_of(&self, latest: &Revision) -> Option<Base> {
        let plate = self.window.plate(&latest.job).await?;
        Some(Base {
            object: plate.job_object?,
            path: plate.path,
            version: Some(plate.version),
        })
    }

    /// Checks `edits` on `base` and keeps them as a revision, handed aside to the window that
    /// handed the plate over: the answer does not wait for it (ADR 0467).
    async fn checked_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        (base, edits): (Base, Vec<Edit>),
        against: &[String],
        ctx: &Context,
    ) -> Result<Reply> {
        let (outcome, written) = self
            .check_edits(&base, &edits, &recipe, against, ctx)
            .await?;
        if !outcome.reported() {
            let _ = std::fs::remove_file(&written);
            return Ok(not_applied(Some(latest), &outcome));
        }
        let kept = self.keep_on_plate((latest, recipe, note), (&edits, &written), &outcome);
        // Kept, the plate was moved into the job; anything left here was not.
        let _ = std::fs::remove_file(&written);
        let (revision, mut reply) = kept?;
        self.tell_untold(&revision.job, &mut reply);
        if let Some(version) = base.version {
            self.apply_aside(&revision, edits, version);
            reply.body["window"] = pending();
        }
        Ok(reply)
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

    /// Keeps the revision checked as its plate and edits.
    fn keep_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        (edits, written): (&[Edit], &std::path::Path),
        outcome: &Outcome,
    ) -> Result<(Revision, Reply)> {
        let revision = self
            .jobs
            .commit(&latest.job, (recipe, None), &outcome.report)?;
        let (job, number) = (&revision.job, revision.number);
        unkept("the edits", self.jobs.keep_edits(job, number, edits));
        unkept("the plate", self.jobs.keep_plate(job, number, written));
        let reply = revised(&revision, outcome, note);
        Ok((revision, reply))
    }
}
