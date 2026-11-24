//! A revision started, changed, checked and kept, each check one run of the command line.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use rmcp::model::ProgressNotificationParam;

use super::reply::{Reply, not_applied, reported, revised};
use super::{Context, Server, plate_edits};
use crate::jobs::{Jobs, Revision};
use crate::params::{Model, Start, absolute, plain};
use crate::recipe::{Recipe, Repair};
use crate::runner::Outcome;

/// What a job of a project answers with first.
const PROJECT: &str = "a project the window saved: its models, poses, hollows and supports \
    stand as saved, and the tools that shape them are refused; check, render, set_slicing, \
    write_file and send_to_printer work. To prepare a model anew, start a job from its file.";

/// How often a client hears that a long run is still going, so it does not time out.
const BEAT: Duration = Duration::from_secs(5);

impl Server {
    pub(super) async fn inspect(&self, model: Model, ctx: &Context) -> Result<Reply> {
        absolute(&model.path, "the model")?;
        if Recipe::bare(model.path.clone(), String::new(), String::new()).is_project() {
            bail!(
                "{} is a project the window saved; start_job checks it as it stands",
                model.path.display()
            );
        }
        let mut args = vec!["inspect".into(), model.path.display().to_string()];
        args.extend(model.repair.map(Repair::flag));
        let outcome = self.watched(&args, ctx).await?;
        Ok(reported(&outcome, outcome.report.clone()))
    }

    pub(super) async fn start(&self, start: Start, ctx: &Context) -> Result<Reply> {
        absolute(&start.model, "the model")?;
        let (printer, resin) = machine_of(start.printer, start.resin)?;
        let mut recipe = Recipe {
            prepare: start.prepare,
            repair: start.repair,
            ..Recipe::bare(start.model, printer, resin)
        };
        if recipe.is_project() {
            if recipe.repair.is_some() {
                bail!("a project's models are repaired in the window, as they were saved");
            }
            recipe.prepare = false;
        }
        let (outcome, steps, plate) = self.checked(&recipe, &[], ctx).await?;
        if !outcome.reported() {
            return Ok(not_applied(None, &outcome));
        }
        let autopilot = if recipe.is_project() {
            None
        } else if recipe.prepare && recipe.repair.is_none() {
            Some(outcome.report.clone())
        } else {
            let alone = Recipe {
                prepare: true,
                ..Recipe::bare(
                    recipe.model.clone(),
                    recipe.printer.clone(),
                    recipe.resin.clone(),
                )
            };
            Some(self.estimate(&alone, &[], ctx).await?.report)
        };
        let job = Jobs::name_for(&recipe);
        let note = recipe.is_project().then_some(PROJECT);
        let revision = self.jobs.commit(&job, (recipe, steps), &outcome.report)?;
        self.keep_plate(&revision, plate);
        if let Some(autopilot) = &autopilot {
            self.jobs.keep_autopilot(&job, autopilot)?;
        }
        Ok(self.kept(&revision, &outcome, note).await)
    }

    /// Changes the job's latest recipe, checks it, and keeps it as the next revision only when
    /// the command line took it. The job's lock is held throughout.
    pub(super) async fn edit(
        &self,
        job: &str,
        ctx: &Context,
        change: impl FnOnce(&mut Recipe) -> Option<&'static str>,
    ) -> Result<Reply> {
        let lock = self.jobs.lock(job);
        let _held = lock.lock().await;
        let latest = self.jobs.open(job, None)?;
        let mut recipe = latest.recipe.clone();
        let undone = self.follow_window_undo(&latest, &mut recipe).await;
        let note = match (change(&mut recipe), undone) {
            (Some(asked), Some(undone)) => Some(format!("{asked}; {undone}")),
            (asked, undone) => asked.or(undone).map(str::to_owned),
        };
        recipe.check_paths()?;
        let against = self.jobs.against(job, Some(latest.number));
        let asked = (&latest, recipe, note.as_deref());
        let recipe = match self.edit_on_plate(asked, &against, ctx).await {
            Ok(reply) => return reply,
            Err(recipe) => *recipe,
        };
        let (outcome, steps, plate) = self.checked(&recipe, &against, ctx).await?;
        if !outcome.reported() {
            return Ok(not_applied(Some(&latest), &outcome));
        }
        let revision = self.jobs.commit(job, (recipe, steps), &outcome.report)?;
        self.keep_plate(&revision, plate);
        Ok(self.kept(&revision, &outcome, note.as_deref()).await)
    }

    /// A check of `recipe` and the steps it ran, as the command line wrote them out, and the
    /// plate it left where it only places its model (ADR 0466).
    pub(super) async fn checked(
        &self,
        recipe: &Recipe,
        against: &[String],
        ctx: &Context,
    ) -> Result<(Outcome, Option<String>, Option<PathBuf>)> {
        let out = self.jobs.scratch_steps()?;
        let mut args = vec!["estimate".to_owned()];
        args.extend(recipe.flags());
        args.extend(against.iter().cloned());
        args.push(format!("--recipe-out={}", out.display()));
        let plate = if recipe.is_project() {
            None
        } else {
            Some(self.jobs.scratch_plate()?)
        };
        if let Some(plate) = &plate {
            args.push(format!("--project-out={}", plate.display()));
        }
        let outcome = self.watched(&args, ctx).await?;
        let steps = std::fs::read_to_string(&out).ok();
        // A scratch file left behind is overwritten by no other run: its name is this run's.
        let _ = std::fs::remove_file(&out);
        Ok((outcome, steps, plate.filter(|plate| plate.is_file())))
    }

    /// Keeps the plate a check wrote as the revision's, which the window is shown.
    pub(super) fn keep_plate(&self, revision: &Revision, plate: Option<PathBuf>) {
        if let Some(plate) = plate {
            let kept = (self.jobs).keep_plate(&revision.job, revision.number, &plate);
            plate_edits::unkept("the plate", kept);
        }
    }

    /// The answer for a revision just kept: what the open window, if any, said of it.
    pub(super) async fn kept(
        &self,
        revision: &Revision,
        outcome: &Outcome,
        note: Option<&str>,
    ) -> Reply {
        let mut reply = revised(revision, outcome, note);
        // The window reads a job's model in beside the plate; a project would take its place.
        if revision.recipe.is_project() {
            return reply;
        }
        if let Some(applied) = self.edits_in_window(revision, &outcome.report).await {
            reply.body["window"] = applied;
            return reply;
        }
        let plate = self.jobs.shown_plate(revision).ok();
        if let Some(plate) = plate
            && let Some(shown) = self.window.show(revision, &plate, false).await
        {
            reply.body["window"] = shown;
        }
        reply
    }

    pub(super) async fn estimate(
        &self,
        recipe: &Recipe,
        against: &[String],
        ctx: &Context,
    ) -> Result<Outcome> {
        let mut args = vec!["estimate".to_owned()];
        args.extend(recipe.flags());
        args.extend(against.iter().cloned());
        self.watched(&args, ctx).await
    }

    /// Runs the command line, telling the client every `BEAT` that it still runs, and stopping
    /// it when the client cancels the call.
    pub(super) async fn watched(&self, args: &[String], ctx: &Context) -> Result<Outcome> {
        let token = ctx.meta.get_progress_token();
        let run = self.cli.run(args);
        tokio::pin!(run);
        let mut beat = tokio::time::interval(BEAT);
        beat.tick().await;
        let mut seconds = 0;
        loop {
            tokio::select! {
                outcome = &mut run => return outcome,
                () = ctx.ct.cancelled() => bail!("the client cancelled the call"),
                _ = beat.tick() => {
                    seconds += BEAT.as_secs();
                    if let Some(token) = &token {
                        let progress = ProgressNotificationParam::new(token.clone(), seconds as f64)
                            .with_message(format!("encrust has run {seconds} s"));
                        // A client that misses a beat still gets the answer.
                        let _ = ctx.peer.notify_progress(progress).await;
                    }
                }
            }
        }
    }
}

/// The printer and resin a call names, both or neither: none is the standard printer with the
/// stock resin (ADR 0475).
pub(super) fn machine_of(
    printer: Option<String>,
    resin: Option<String>,
) -> Result<(String, String)> {
    match (printer, resin) {
        (None, None) => Ok((String::new(), String::new())),
        (Some(printer), Some(resin)) => {
            plain(&printer, "the printer")?;
            plain(&resin, "the resin")?;
            Ok((printer, resin))
        }
        _ => bail!("name the printer and the resin together, or neither for the standard ones"),
    }
}
