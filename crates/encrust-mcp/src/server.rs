//! The tools a client sees, each one or two runs of the command line over a job's revision.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
    ProgressNotificationParam, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde_json::json;

use crate::agents::Presence;
use crate::findings::{MOST_MARKED, findings};
use crate::jobs::{Jobs, Revision};
use crate::params::{
    ComparePoses, CreateResin, ImportResin, JobRef, ListProfiles, ListRecipes, Model, Render,
    ReportPrint, Revert, SaveRecipe, SendFile, SetDrains, SetHollow, SetPose, SetRepair, SetShape,
    SetSlicing, SetSupports, Start, UseRecipe, WriteFile, WriteTestPrint, absolute, plain,
};
use crate::recipe::{Recipe, Repair};
use crate::runner::{Cli, Outcome};
use crate::trace::{Trace, body_of};
use crate::window::Window;

mod after;
mod asked;
mod library;
mod plate_edits;
mod poses;
mod reply;
mod shown;
mod written;

use reply::{Reply, answer, not_applied, reported, revised};

const INSTRUCTIONS: &str = include_str!("instructions.md");

/// What a job of a project answers with first.
const PROJECT: &str = "a project the window saved: its models, poses, hollows and supports \
    stand as saved, and the tools that shape them are refused; check, render, set_slicing, \
    write_file and send_to_printer work. To prepare a model anew, start a job from its file.";

/// How often a client hears that a long run is still going, so it does not time out.
const BEAT: Duration = Duration::from_secs(5);

/// The server: the command line it drives, where its jobs are kept and where its calls are
/// traced.
#[derive(Debug, Clone)]
pub struct Server {
    cli: Cli,
    jobs: Jobs,
    trace: Trace,
    window: Window,
    presence: Presence,
    tool_router: ToolRouter<Self>,
}

type Answer = Result<CallToolResult, rmcp::ErrorData>;
type Context = RequestContext<RoleServer>;

#[tool_router]
impl Server {
    pub fn new(cli: Cli, jobs: Jobs, (trace, window, presence): (Trace, Window, Presence)) -> Self {
        Self {
            cli,
            jobs,
            trace,
            window,
            presence,
            tool_router: Self::tool_router(),
        }
    }

    /// The printers, the user's resins and the support profiles there are to choose from.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn list_profiles(
        &self,
        Parameters(list): Parameters<ListProfiles>,
        ctx: Context,
    ) -> Answer {
        Ok(answer(self.profiles(list, &ctx).await))
    }

    /// Takes in the user's resin from a file that printed well with it, another slicer's
    /// export or a resin TOML, as one of their own profiles; answers with its id.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn import_resin(
        &self,
        Parameters(import): Parameters<ImportResin>,
        ctx: Context,
    ) -> Answer {
        Ok(answer(self.import(import, &ctx).await))
    }

    /// Makes the user's resin from the numbers on its bottle or its maker's page, set up on
    /// one printer; answers with its id and what the printer advises against.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn create_resin(
        &self,
        Parameters(create): Parameters<CreateResin>,
        ctx: Context,
    ) -> Answer {
        Ok(answer(self.new_resin(create, &ctx).await))
    }

    /// Writes one of the project's test prints as an STL, to prepare and print like any model:
    /// the exposure check tells whether a resin's exposure is right.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn write_test_print(
        &self,
        Parameters(write): Parameters<WriteTestPrint>,
        ctx: Context,
    ) -> Answer {
        let written = async {
            absolute(&write.folder, "the folder")?;
            let args = [
                "test-print".into(),
                write.print.name().into(),
                format!("--output={}", write.folder.display()),
            ];
            let outcome = self.watched(&args, &ctx).await?;
            Ok(reported(&outcome, outcome.report.clone()))
        };
        Ok(answer(written.await))
    }

    /// The recipes a job can be prepared by, built-in and the user's, each with its steps as
    /// TOML: the autopilot `prepare`, `miniatures`, `fix-issues` and the user's own.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn list_recipes(&self, _: Parameters<ListRecipes>, ctx: Context) -> Answer {
        Ok(answer(self.recipes(&ctx).await))
    }

    /// Prepares the job by another recipe — one fitting what the print is for — and checks it.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn use_recipe(&self, Parameters(set): Parameters<UseRecipe>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.prepare |= set.recipe.is_none();
            recipe.steps_by = set.recipe;
            None
        });
        Ok(answer(edited.await))
    }

    /// Keeps a job's way of preparing, or one the user described, as a recipe of their own,
    /// which the window then runs as Prepare for print with no chat; answers with its id.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn save_recipe(&self, Parameters(save): Parameters<SaveRecipe>, ctx: Context) -> Answer {
        Ok(answer(self.keep_recipe(save, &ctx).await))
    }

    /// What a model file is: its size, shells, holes, defects, and what a repair would do.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn inspect_model(&self, Parameters(model): Parameters<Model>, ctx: Context) -> Answer {
        Ok(answer(self.inspect(model, &ctx).await))
    }

    /// Starts preparing a model for the printer and resin the user named, and checks it as it
    /// stands.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn start_job(&self, Parameters(start): Parameters<Start>, ctx: Context) -> Answer {
        Ok(answer(self.start(start, &ctx).await))
    }

    /// Sets how the model stands — the orientation search, or a turn — and checks the result.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_pose(&self, Parameters(set): Parameters<SetPose>, ctx: Context) -> Answer {
        let mut set = set;
        if let Some(pose) = set.pose.as_mut() {
            let seated = self
                .jobs
                .open(&set.job, None)
                .and_then(|latest| self.seat_keep_clear(pose, &latest));
            if let Err(error) = seated {
                return Ok(answer(Err(error)));
            }
        }
        let edited = self.edit(&set.job, &ctx, |recipe| {
            let had_drains = recipe
                .drains
                .as_mut()
                .is_some_and(|drains| !std::mem::take(&mut drains.at_mm).is_empty());
            recipe.pose = set.pose;
            had_drains.then_some(
                "the drain points were cleared: they were on the old pose; automatic drains stay",
            )
        });
        Ok(answer(edited.await))
    }

    /// The search's best ways to stand the model, each with what it prints and drawn, to
    /// choose one by what the model is; `set_pose` with `pick` takes it.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn compare_poses(
        &self,
        Parameters(compare): Parameters<ComparePoses>,
        ctx: Context,
    ) -> Answer {
        Ok(self
            .poses(compare, &ctx)
            .await
            .unwrap_or_else(|error| answer(Err(error))))
    }

    /// Sets the model's cavity and infill, or none, and checks for resin left trapped.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_hollow(&self, Parameters(set): Parameters<SetHollow>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.hollow = set.hollow;
            None
        });
        Ok(answer(edited.await))
    }

    /// Sets the drain holes that let resin out of the cavity, and checks what stays trapped.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_drains(&self, Parameters(set): Parameters<SetDrains>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.drains = set.drains;
            None
        });
        Ok(answer(edited.await))
    }

    /// Sets the supports the model stands on, and checks what is left hanging.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_supports(&self, Parameters(set): Parameters<SetSupports>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.supports = set.supports;
            None
        });
        Ok(answer(edited.await))
    }

    /// Sets the model's size and the modelling done to it — combined with others, labelled,
    /// cut with a joint, split, a part cut off — and a texture pressed in; checks the result.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_shape(&self, Parameters(set): Parameters<SetShape>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.modelling = set.modelling;
            recipe.relief = set.relief;
            None
        });
        Ok(answer(edited.await))
    }

    /// Sets the repair the model gets and the faces and shells changed by hand, by the numbers
    /// `inspect_model` gives; checks the result.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_repair(&self, Parameters(set): Parameters<SetRepair>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.repair = set.repair;
            recipe.mend = set.mend;
            None
        });
        Ok(answer(edited.await))
    }

    /// Sets how the model is sliced and its masks drawn — layer height, adaptive layers,
    /// exposure bands, anti-aliasing, islands taken out — and checks the result.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn set_slicing(&self, Parameters(set): Parameters<SetSlicing>, ctx: Context) -> Answer {
        let edited = self.edit(&set.job, &ctx, |recipe| {
            recipe.slicing = set.slicing;
            None
        });
        Ok(answer(edited.await))
    }

    /// Checks a revision as it would print: islands, peel, suction cups, trapped resin, fit,
    /// time and resin.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn check(&self, Parameters(at): Parameters<JobRef>, ctx: Context) -> Answer {
        let checked = async {
            let revision = self.jobs.open(&at.job, at.revision)?;
            let against = self
                .jobs
                .against(&revision.job, revision.number.checked_sub(1));
            let outcome = self.estimate(&revision.recipe, &against, &ctx).await?;
            Ok(revised(&revision, &outcome, None))
        };
        Ok(answer(checked.await))
    }

    /// Makes an earlier revision the latest again, as a new revision, and checks it.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn revert(&self, Parameters(revert): Parameters<Revert>, ctx: Context) -> Answer {
        let reverted = async {
            let lock = self.jobs.lock(&revert.job);
            let _held = lock.lock().await;
            let old = self.jobs.open(&revert.job, Some(revert.revision))?;
            let latest = self.jobs.open(&revert.job, None)?;
            let against = self.jobs.against(&revert.job, Some(latest.number));
            let (outcome, steps) = self.checked(&old.recipe, &against, &ctx).await?;
            let kept = (old.recipe, steps);
            let revision = self.jobs.commit(&revert.job, kept, &outcome.report)?;
            Ok(self.kept(&revision, &outcome, None).await)
        };
        Ok(answer(reverted.await))
    }

    /// Writes the printer file of a revision. Refused while its check names defects, unless the
    /// user accepted them.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        open_world_hint = false
    ))]
    async fn write_file(&self, Parameters(write): Parameters<WriteFile>, ctx: Context) -> Answer {
        Ok(answer(self.write(write, &ctx).await))
    }

    /// Draws a revision as it would be built — supports, drain holes and all — from the sides
    /// asked, with the check's findings marked, as PNG pictures.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn render(&self, Parameters(render): Parameters<Render>, ctx: Context) -> Answer {
        Ok(self
            .pictures(render, &ctx)
            .await
            .unwrap_or_else(|error| answer(Err(error))))
    }

    /// What a print of the job did, as the user saw it: each symptom's likely causes read
    /// against the check of the revision printed, each with the edit to try; kept with the job.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn report_print(
        &self,
        Parameters(print): Parameters<ReportPrint>,
        ctx: Context,
    ) -> Answer {
        Ok(answer(self.after_print(print, &ctx).await))
    }

    /// Shows a revision on the plate of the user's Encrust window, starting the window if it
    /// is closed: when the user asks to see or adjust the model there.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        open_world_hint = false
    ))]
    async fn show_in_window(&self, Parameters(at): Parameters<JobRef>, _: Context) -> Answer {
        Ok(answer(self.shown(at).await))
    }

    /// Sends a written file to a printer on the local network, and starts it if asked.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        open_world_hint = true
    ))]
    async fn send_to_printer(
        &self,
        Parameters(send): Parameters<SendFile>,
        ctx: Context,
    ) -> Answer {
        Ok(answer(self.send(send, &ctx).await))
    }
}

impl Server {
    async fn inspect(&self, model: Model, ctx: &Context) -> Result<Reply> {
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

    async fn start(&self, start: Start, ctx: &Context) -> Result<Reply> {
        absolute(&start.model, "the model")?;
        for (value, what) in [(&start.printer, "the printer"), (&start.resin, "the resin")] {
            plain(value, what)?;
        }
        let mut recipe = Recipe {
            prepare: start.prepare,
            repair: start.repair,
            ..Recipe::bare(start.model, start.printer, start.resin)
        };
        if recipe.is_project() {
            if recipe.repair.is_some() {
                bail!("a project's models are repaired in the window, as they were saved");
            }
            recipe.prepare = false;
        }
        let (outcome, steps) = self.checked(&recipe, &[], ctx).await?;
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
        if let Some(autopilot) = &autopilot {
            self.jobs.keep_autopilot(&job, autopilot)?;
        }
        Ok(self.kept(&revision, &outcome, note).await)
    }

    /// Changes the job's latest recipe, checks it, and keeps it as the next revision only when
    /// the command line took it. The job's lock is held throughout.
    async fn edit(
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
        let (outcome, steps) = self.checked(&recipe, &against, ctx).await?;
        if !outcome.reported() {
            return Ok(not_applied(Some(&latest), &outcome));
        }
        let revision = self.jobs.commit(job, (recipe, steps), &outcome.report)?;
        Ok(self.kept(&revision, &outcome, note.as_deref()).await)
    }

    /// A check of `recipe` and the steps it ran, as the command line wrote them out.
    async fn checked(
        &self,
        recipe: &Recipe,
        against: &[String],
        ctx: &Context,
    ) -> Result<(Outcome, Option<String>)> {
        let out = self.jobs.scratch_steps()?;
        let mut args = vec!["estimate".to_owned()];
        args.extend(recipe.flags());
        args.extend(against.iter().cloned());
        args.push(format!("--recipe-out={}", out.display()));
        let outcome = self.watched(&args, ctx).await?;
        let steps = std::fs::read_to_string(&out).ok();
        // A scratch file left behind is overwritten by no other run: its name is this run's.
        let _ = std::fs::remove_file(&out);
        Ok((outcome, steps))
    }

    /// The answer for a revision just kept: what the open window, if any, said of running its
    /// steps, and what the steps leave out.
    async fn kept(&self, revision: &Revision, outcome: &Outcome, note: Option<&str>) -> Reply {
        let mut reply = revised(revision, outcome, note);
        // The window reads a job's model in beside the plate; a project would take its place.
        if revision.recipe.is_project() {
            return reply;
        }
        if let Some(applied) = self.edits_in_window(revision).await {
            reply.body["window"] = applied;
            return reply;
        }
        if let Some(shown) = self.window.show(revision, false).await {
            reply.body["window"] = shown;
            reply.body["unshown"] = json!(revision.recipe.unshown());
        }
        reply
    }

    async fn estimate(
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
    async fn watched(&self, args: &[String], ctx: &Context) -> Result<Outcome> {
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

/// The client's own name for itself: from the request's `_meta`, as a client of the stateless
/// protocol sends it, else from its handshake, else `claude-code` where Claude Code, which
/// may send neither, marks the processes it starts.
fn client_name(context: &Context) -> Option<String> {
    context
        .meta
        .client_info()
        .map(|info| info.name)
        .or_else(|| {
            context
                .peer
                .peer_info()
                .map(|info| info.client_info.name.clone())
        })
        .or_else(|| {
            (std::env::var("CLAUDECODE").as_deref() == Ok("1")).then(|| "claude-code".to_owned())
        })
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "rmcp's tool_handler expands to it"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: Context,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let started = Instant::now();
        let tool = request.name.clone();
        let client = client_name(&context);
        let named = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("job")?.as_str().map(str::to_owned));
        self.presence
            .begin(client.as_deref(), &tool, named.as_deref());
        let response = self
            .tool_router
            .call(ToolCallContext::new(self, request, context))
            .await;
        let answered = body_of(&response)["job"].as_str().map(str::to_owned);
        self.presence.end(answered.or(named).as_deref());
        self.trace.record(&tool, started.elapsed(), &response).await;
        response
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("encrust", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
