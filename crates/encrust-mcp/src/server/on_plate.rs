//! A revision made of edits, checked on the open window's plate with what the user did there,
//! and kept as that plate and its edits (ADR 0466, `docs/design/window-link.md`).

use std::path::PathBuf;

use anyhow::Result;
use core_edit::{Edit, ObjectId};
use serde_json::json;

use super::Server;
use super::asked::{Area, asked_after_check};
use super::plate_edits::{fit_drift_of, unkept, usable_area_of};
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
        let (drift, area) = self.earlier_check(latest, &recipe);
        let ask = |object| asked_after_check(&recipe, &latest.recipe, object, (None, drift, area));
        let asked = ask(ObjectId::UNSET);
        if asked.as_ref().is_none_or(Vec::is_empty) {
            return Err(Box::new(recipe));
        }
        let Some(base) = self.base_of(latest).await else {
            return Err(Box::new(recipe));
        };
        let Some(edits) = ask(base.object) else {
            return Err(Box::new(recipe));
        };
        Ok(self
            .checked_on_plate((latest, recipe, note), &base, &edits, against, ctx)
            .await)
    }

    /// The fit drift and usable plate area the previous revision's check reported, where it ran
    /// on the printer and resin `recipe` names: this revision's check has not run yet.
    fn earlier_check(&self, latest: &Revision, recipe: &Recipe) -> (Option<f32>, Option<Area>) {
        let same = latest.recipe.printer == recipe.printer && latest.recipe.resin == recipe.resin;
        let Some(report) = self.jobs.report(latest).ok().filter(|_| same) else {
            return (None, None);
        };
        (fit_drift_of(&report), usable_area_of(&report))
    }

    /// The open window's plate with the job's model on it, which holds whatever the user did
    /// there. Without a window nothing but the job changes the plate, and its recipe stays the
    /// way it is checked and kept, steps and all.
    async fn base_of(&self, latest: &Revision) -> Option<Base> {
        let plate = self.window.plate(&latest.job).await?;
        Some(Base {
            object: plate.job_object?,
            path: plate.path,
            version: Some(plate.version),
        })
    }

    async fn checked_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        base: &Base,
        edits: &[Edit],
        against: &[String],
        ctx: &Context,
    ) -> Result<Reply> {
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
        let outcome = outcome?;
        if !outcome.reported() {
            return Ok(not_applied(Some(latest), &outcome));
        }
        let revision = self
            .jobs
            .commit(&latest.job, (recipe, None), &outcome.report)?;
        unkept(
            "the edits",
            self.jobs.keep_edits(&revision.job, revision.number, edits),
        );
        let kept = self
            .jobs
            .keep_plate(&revision.job, revision.number, &written);
        unkept("the plate", kept);
        let mut reply = revised(&revision, &outcome, note);
        if let Some(version) = base.version {
            let at = Some((version, base.object));
            let answer = self.handed_to_window(&revision, at, |_| Some(edits.to_vec()));
            reply.body["window"] = answer.await.unwrap_or_else(
                || json!({"applied": false, "why_not": "the window did not answer"}),
            );
        }
        Ok(reply)
    }
}
