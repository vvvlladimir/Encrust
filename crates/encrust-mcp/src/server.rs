//! The tools a client sees, each one or two runs of the command line over a job's revision.

use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProgressNotificationParam, ServerCapabilities,
    ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde_json::{Value, json};

use crate::findings::{MOST_MARKED, findings};
use crate::jobs::{Jobs, Revision};
use crate::params::{
    JobRef, ListProfiles, Model, Render, Revert, SendFile, SetDrains, SetHollow, SetPose,
    SetSupports, Start, View, WriteFile, absolute, plain,
};
use crate::recipe::{Recipe, Repair};
use crate::runner::{Cli, Outcome};

const INSTRUCTIONS: &str = include_str!("instructions.md");

/// How often a client hears that a long run is still going, so it does not time out.
const BEAT: Duration = Duration::from_secs(5);

/// The server: the command line it drives and where its jobs are kept.
#[derive(Debug, Clone)]
pub struct Server {
    cli: Cli,
    jobs: Jobs,
    tool_router: ToolRouter<Self>,
}

type Answer = Result<CallToolResult, rmcp::ErrorData>;
type Context = RequestContext<RoleServer>;

#[tool_router]
impl Server {
    pub fn new(cli: Cli, jobs: Jobs) -> Self {
        Self {
            cli,
            jobs,
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

    /// Checks a revision as it would print: islands, peel, suction cups, trapped resin, fit,
    /// time and resin.
    #[tool(annotations(read_only_hint = true, open_world_hint = false))]
    async fn check(&self, Parameters(at): Parameters<JobRef>, ctx: Context) -> Answer {
        let checked = async {
            let revision = self.jobs.open(&at.job, at.revision)?;
            let outcome = self.estimate(&revision.recipe, &ctx).await?;
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
            let revision = self.jobs.commit(&revert.job, old.recipe)?;
            let outcome = self.estimate(&revision.recipe, &ctx).await?;
            Ok(revised(&revision, &outcome, None))
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

/// What a tool answers: a body, and whether the client should read it as a failure.
struct Reply {
    body: Value,
    failed: bool,
}

impl Server {
    async fn profiles(&self, list: ListProfiles, ctx: &Context) -> Result<Reply> {
        let outcome = self
            .watched(&["profiles".into(), "list".into()], ctx)
            .await?;
        Ok(reported(
            &outcome,
            trim_profiles(outcome.report.clone(), list.printer_query.as_deref()),
        ))
    }

    async fn inspect(&self, model: Model, ctx: &Context) -> Result<Reply> {
        absolute(&model.path, "the model")?;
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
        let recipe = Recipe {
            repair: start.repair,
            ..Recipe::bare(start.model, start.printer, start.resin)
        };
        let outcome = self.estimate(&recipe, ctx).await?;
        if !outcome.reported() {
            return Ok(not_applied(None, &outcome));
        }
        let job = Jobs::name_for(&recipe);
        let revision = self.jobs.commit(&job, recipe)?;
        Ok(revised(&revision, &outcome, None))
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
        let note = change(&mut recipe);
        if let Some(supports) = &recipe.supports {
            plain(&supports.profile, "the support profile")?;
        }
        let outcome = self.estimate(&recipe, ctx).await?;
        if !outcome.reported() {
            return Ok(not_applied(Some(&latest), &outcome));
        }
        let revision = self.jobs.commit(job, recipe)?;
        Ok(revised(&revision, &outcome, note))
    }

    async fn write(&self, write: WriteFile, ctx: &Context) -> Result<Reply> {
        absolute(&write.output, "the output")?;
        let revision = self.jobs.open(&write.job, write.revision)?;
        let mut args = vec!["slice".to_owned()];
        args.extend(revision.recipe.flags());
        args.push(format!("--output={}", write.output.display()));
        if !write.accept_defects {
            args.push("--strict".into());
        }
        let outcome = self.watched(&args, ctx).await?;
        let written = outcome.exit_code == 0;
        let body = json!({
            "job": revision.job,
            "revision": revision.number,
            "written": written,
            "output": written.then(|| write.output.display().to_string()),
            "why_not": (!written).then_some(if outcome.reported() {
                "the check names defects; nothing was written"
            } else {
                "the command line refused the run; nothing was written"
            }),
            "report": outcome.report,
        });
        Ok(Reply {
            body,
            failed: !written,
        })
    }

    /// The pictures of a revision, and a JSON list of each one with the findings it marks.
    async fn pictures(&self, render: Render, ctx: &Context) -> Result<CallToolResult> {
        let revision = self.jobs.open(&render.job, render.revision)?;
        let views = render
            .views
            .unwrap_or_else(|| vec![View::Iso, View::Bottom]);
        if views.is_empty() {
            bail!("name at least one view");
        }
        let size = render.size_px.unwrap_or(512).clamp(64, 1024);
        let marked = if render.mark_findings.unwrap_or(true) {
            let outcome = self.estimate(&revision.recipe, ctx).await?;
            findings(&outcome.report)
        } else {
            Vec::new()
        };
        let dir = self.jobs.pictures_dir(&revision)?;
        let names: Vec<&str> = views.iter().map(|view| view.name()).collect();
        let mut args = vec!["render".to_owned()];
        args.extend(revision.recipe.flags());
        args.push(format!("--view={}", names.join(",")));
        args.push(format!("--size={size}"));
        args.push(format!("--output={}", dir.display()));
        for finding in marked.iter().take(MOST_MARKED) {
            let [x, y, z] = finding.at_mm;
            args.push(format!("--mark={x},{y},{z}"));
        }
        let outcome = self.watched(&args, ctx).await?;
        if outcome.exit_code != 0 {
            return Ok(answer(Ok(reported(&outcome, outcome.report.clone()))));
        }
        let mut content = vec![ContentBlock::text(
            json!({
                "job": revision.job,
                "revision": revision.number,
                "findings": marked,
                "unmarked": marked.len().saturating_sub(MOST_MARKED),
                "pictures": outcome.report["pictures"],
            })
            .to_string(),
        )];
        for picture in outcome.report["pictures"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            let path = picture["path"]
                .as_str()
                .context("a picture names its file")?;
            let bytes = tokio::fs::read(path)
                .await
                .with_context(|| format!("reading {path}"))?;
            content.push(ContentBlock::image(BASE64.encode(bytes), "image/png"));
        }
        Ok(CallToolResult::success(content))
    }

    async fn send(&self, send: SendFile, ctx: &Context) -> Result<Reply> {
        absolute(&send.file, "the file")?;
        plain(&send.address, "the printer's address")?;
        let mut args = vec![
            "printer".into(),
            "send".into(),
            send.file.display().to_string(),
            send.address,
        ];
        if send.start {
            args.push("--start".into());
        }
        let outcome = self.watched(&args, ctx).await?;
        Ok(reported(&outcome, outcome.report.clone()))
    }

    async fn estimate(&self, recipe: &Recipe, ctx: &Context) -> Result<Outcome> {
        let mut args = vec!["estimate".to_owned()];
        args.extend(recipe.flags());
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

fn revised(revision: &Revision, outcome: &Outcome, note: Option<&str>) -> Reply {
    Reply {
        body: json!({
            "job": revision.job,
            "revision": revision.number,
            "recipe": revision.recipe,
            "note": note,
            "exit_code": outcome.exit_code,
            "report": outcome.report,
        }),
        failed: false,
    }
}

fn not_applied(latest: Option<&Revision>, outcome: &Outcome) -> Reply {
    Reply {
        body: json!({
            "applied": false,
            "why_not": "the command line refused the recipe",
            "latest_revision": latest.map(|revision| revision.number),
            "exit_code": outcome.exit_code,
            "report": outcome.report,
        }),
        failed: true,
    }
}

fn reported(outcome: &Outcome, body: Value) -> Reply {
    Reply {
        body,
        failed: !outcome.reported(),
    }
}

/// Printers cut to what choosing one takes, and to the query; the catalogue is long.
fn trim_profiles(mut listing: Value, query: Option<&str>) -> Value {
    let query = query.map(str::to_lowercase);
    if let Some(printers) = listing.get_mut("printers").and_then(Value::as_array_mut) {
        let kept: Vec<Value> = printers
            .iter()
            .filter(|printer| {
                query.as_deref().is_none_or(|query| {
                    ["id", "manufacturer", "name"].iter().any(|key| {
                        printer[*key].as_str().is_some_and(|text| text.to_lowercase().contains(query))
                    })
                })
            })
            .map(|printer| {
                json!({
                    "id": printer["id"],
                    "name": format!("{} {}", printer["manufacturer"].as_str().unwrap_or_default(), printer["name"].as_str().unwrap_or_default()),
                    "build_volume_mm": printer["build_volume_mm"],
                    "user": printer["user"],
                })
            })
            .collect();
        *printers = kept;
    }
    listing
}

fn answer(reply: Result<Reply>) -> CallToolResult {
    let (body, failed) = match reply {
        Ok(reply) => (reply.body, reply.failed),
        Err(error) => (json!({ "error": format!("{error:#}") }), true),
    };
    let content = vec![ContentBlock::text(body.to_string())];
    if failed {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "rmcp's tool_handler expands to it"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("encrust", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
