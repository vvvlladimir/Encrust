//! A revision made of edits, checked on the open window's plate with what the user did there,
//! and kept as that plate and its edits (ADR 0466, `docs/design/window-link.md`).

use std::path::PathBuf;

use anyhow::Result;
use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use crate::runner::Outcome;

use super::Server;
use super::asked::{Area, Checked, asked_after_check};
use super::plate_edits::{GAVE_UP, REBUILDS, cut_object, fit_drift_of, unkept, usable_area_of};
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
        let checked = (None, drift, area);
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
                return Ok(not_applied(Some(latest), &outcome));
            }
            let answer = match base.version {
                Some(version) => Some(self.window.apply(&latest.job, &edits, version).await),
                None => None,
            };
            let stale = answer
                .as_ref()
                .is_some_and(|a| a.as_ref().is_some_and(|a| a["stale"] == true));
            if stale && rebuilt < REBUILDS {
                rebuilt += 1;
                if let Some(next) = self.rebased(latest, &recipe, checked).await {
                    (base, edits) = next;
                    continue;
                }
            }
            let window = answer.map(|answer| match answer {
                Some(_) if stale => json!({"applied": false, "why_not": GAVE_UP}),
                Some(answer) => answer,
                None => json!({"applied": false, "why_not": "the window did not answer"}),
            });
            let kept = (edits.as_slice(), written.as_path(), window);
            return self.keep_on_plate((latest, recipe, note), kept, &outcome);
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
        Ok((outcome?, written))
    }

    /// Keeps the revision checked as its plate and edits, with what the window said of them.
    fn keep_on_plate(
        &self,
        (latest, recipe, note): Asked<'_>,
        (edits, written, window): (&[Edit], &std::path::Path, Option<Value>),
        outcome: &Outcome,
    ) -> Result<Reply> {
        let revision = self
            .jobs
            .commit(&latest.job, (recipe, None), &outcome.report)?;
        let (job, number) = (&revision.job, revision.number);
        unkept("the edits", self.jobs.keep_edits(job, number, edits));
        unkept("the plate", self.jobs.keep_plate(job, number, written));
        let landed = window
            .as_ref()
            .is_some_and(|answer| answer["applied"] == true);
        if let Some(cut) = cut_object(edits).filter(|_| landed) {
            unkept(
                "the window's cut",
                self.jobs.keep_window_cut(job, number, cut),
            );
        }
        let mut reply = revised(&revision, outcome, note);
        if let Some(window) = window {
            reply.body["window"] = window;
        }
        Ok(reply)
    }
}
