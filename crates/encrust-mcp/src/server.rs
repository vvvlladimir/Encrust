//! The tools a client sees, each one or two runs of the command line over a job's revision.

use std::path::PathBuf;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::jobs::{Jobs, Revision};
use crate::recipe::{Drains, Hollow, Pose, Recipe, Supports};
use crate::runner::{Cli, DEFECTS, Outcome};

const INSTRUCTIONS: &str = include_str!("instructions.md");

/// The server: the command line it drives and where its jobs are kept.
#[derive(Debug, Clone)]
pub struct Server {
    cli: Cli,
    jobs: Jobs,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Model {
    /// Absolute path of an STL, OBJ or 3MF file on this computer.
    pub path: PathBuf,
    /// `full`, `close-holes`, `close-small-holes`, `turn`, `wrap` or `deep`; absent for none.
    pub repair: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Start {
    /// Absolute path of an STL, OBJ or 3MF file on this computer.
    pub model: PathBuf,
    /// A printer id `list_profiles` names, or the path of a printer TOML.
    pub printer: String,
    /// A resin id `list_profiles` names, or the path of a resin TOML.
    pub resin: String,
    pub repair: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct JobRef {
    /// The id `start_job` returned.
    pub job: String,
    /// A revision counted from one; absent for the latest.
    pub revision: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPose {
    pub job: String,
    /// Absent stands the model as its file does.
    pub pose: Option<Pose>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetHollow {
    pub job: String,
    /// Absent leaves the model solid.
    pub hollow: Option<Hollow>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDrains {
    pub job: String,
    /// Absent drills no hole.
    pub drains: Option<Drains>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSupports {
    pub job: String,
    /// Absent stands the model on nothing but the plate.
    pub supports: Option<Supports>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Revert {
    pub job: String,
    /// The revision to make the latest again.
    pub revision: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFile {
    pub job: String,
    /// Absolute path of the file to write; its extension picks the format (`.goo`, `.ctb`, …).
    pub output: PathBuf,
    /// Writes even when the check names defects. Only when the user accepted them.
    #[serde(default)]
    pub accept_defects: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendFile {
    /// Absolute path of a file `write_file` wrote.
    pub file: PathBuf,
    /// The printer's IP address or host on the local network.
    pub address: String,
    /// Starts the print once the printer has checked the file.
    #[serde(default)]
    pub start: bool,
}

type Answer = Result<CallToolResult, rmcp::ErrorData>;

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
    #[tool(annotations(read_only_hint = true))]
    async fn list_profiles(&self) -> Answer {
        self.run(&["profiles".into(), "list".into()]).await
    }

    /// What a model file is: its size, shells, holes, defects, and what a repair would do.
    #[tool(annotations(read_only_hint = true))]
    async fn inspect_model(&self, Parameters(model): Parameters<Model>) -> Answer {
        let mut args = vec!["inspect".into(), model.path.display().to_string()];
        if let Some(repair) = model.repair {
            args.push(if repair == "full" {
                "--repair".into()
            } else {
                format!("--repair={repair}")
            });
        }
        self.run(&args).await
    }

    /// Starts preparing a model for a printer and resin, and checks it as it stands.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn start_job(&self, Parameters(start): Parameters<Start>) -> Answer {
        let recipe = Recipe {
            model: start.model,
            printer: start.printer,
            resin: start.resin,
            repair: start.repair,
            pose: None,
            hollow: None,
            drains: None,
            supports: None,
        };
        match self.jobs.start(recipe) {
            Ok(revision) => Ok(self.checked(revision).await),
            Err(error) => Ok(refused(&error)),
        }
    }

    /// Sets how the model stands — the orientation search, or a turn — and checks the result.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn set_pose(&self, Parameters(set): Parameters<SetPose>) -> Answer {
        self.edit(&set.job, |recipe| recipe.pose = set.pose).await
    }

    /// Sets the model's cavity and infill, or none, and checks for resin left trapped.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn set_hollow(&self, Parameters(set): Parameters<SetHollow>) -> Answer {
        self.edit(&set.job, |recipe| recipe.hollow = set.hollow)
            .await
    }

    /// Sets the drain holes that let resin out of the cavity, and checks what stays trapped.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn set_drains(&self, Parameters(set): Parameters<SetDrains>) -> Answer {
        self.edit(&set.job, |recipe| recipe.drains = set.drains)
            .await
    }

    /// Sets the supports the model stands on, and checks what is left hanging.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn set_supports(&self, Parameters(set): Parameters<SetSupports>) -> Answer {
        self.edit(&set.job, |recipe| recipe.supports = set.supports)
            .await
    }

    /// Checks a revision as it would print: islands, peel, suction cups, trapped resin, fit,
    /// time and resin.
    #[tool(annotations(read_only_hint = true))]
    async fn check(&self, Parameters(at): Parameters<JobRef>) -> Answer {
        match self.jobs.open(&at.job, at.revision) {
            Ok(revision) => Ok(self.checked(revision).await),
            Err(error) => Ok(refused(&error)),
        }
    }

    /// Makes an earlier revision the latest again, as a new revision.
    #[tool(annotations(read_only_hint = false, destructive_hint = false))]
    async fn revert(&self, Parameters(revert): Parameters<Revert>) -> Answer {
        let reverted = self
            .jobs
            .open(&revert.job, Some(revert.revision))
            .and_then(|old| self.jobs.next(&revert.job, old.recipe));
        match reverted {
            Ok(revision) => Ok(self.checked(revision).await),
            Err(error) => Ok(refused(&error)),
        }
    }

    /// Writes the printer file of the job's latest revision. Refused while the check names
    /// defects, unless the user accepted them.
    #[tool(annotations(read_only_hint = false, destructive_hint = true))]
    async fn write_file(&self, Parameters(write): Parameters<WriteFile>) -> Answer {
        let revision = match self.jobs.open(&write.job, None) {
            Ok(revision) => revision,
            Err(error) => return Ok(refused(&error)),
        };
        let mut args = vec!["slice".to_owned()];
        args.extend(revision.recipe.flags());
        args.extend(["-o".into(), write.output.display().to_string()]);
        if !write.accept_defects {
            args.push("--strict".into());
        }
        self.run(&args).await
    }

    /// Sends a written file to a printer on the local network, and starts it if asked.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = true,
        open_world_hint = true
    ))]
    async fn send_to_printer(&self, Parameters(send): Parameters<SendFile>) -> Answer {
        let mut args = vec![
            "printer".into(),
            "send".into(),
            send.file.display().to_string(),
            send.address,
        ];
        if send.start {
            args.push("--start".into());
        }
        self.run(&args).await
    }
}

impl Server {
    async fn edit(&self, job: &str, change: impl FnOnce(&mut Recipe)) -> Answer {
        let edited = self.jobs.open(job, None).and_then(|open| {
            let mut recipe = open.recipe;
            change(&mut recipe);
            self.jobs.next(job, recipe)
        });
        match edited {
            Ok(revision) => Ok(self.checked(revision).await),
            Err(error) => Ok(refused(&error)),
        }
    }

    /// Estimates a revision and answers with the revision, its recipe and the report.
    async fn checked(&self, revision: Revision) -> CallToolResult {
        let mut args = vec!["estimate".to_owned()];
        args.extend(revision.recipe.flags());
        let outcome = match self.cli.run(&args).await {
            Ok(outcome) => outcome,
            Err(error) => return refused(&error),
        };
        if let Err(error) = self.jobs.keep_report(&revision, &outcome.report) {
            return refused(&error);
        }
        let answer = json!({
            "job": revision.job,
            "revision": revision.number,
            "recipe": revision.recipe,
            "exit_code": outcome.exit_code,
            "report": outcome.report,
        });
        answered(&outcome, &answer)
    }

    async fn run(&self, args: &[String]) -> Answer {
        match self.cli.run(args).await {
            Ok(outcome) => Ok(answered(&outcome, &outcome.report)),
            Err(error) => Ok(refused(&error)),
        }
    }
}

/// A run that reported is an answer; one that failed outright, rather than finding
/// defects, is a tool error the model reads and can act on.
fn answered(outcome: &Outcome, body: &Value) -> CallToolResult {
    let text = serde_json::to_string_pretty(body).unwrap_or_else(|_| body.to_string());
    let content = vec![ContentBlock::text(text)];
    if outcome.is_done() || outcome.exit_code == DEFECTS {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    }
}

fn refused(error: &anyhow::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("{error:#}"))])
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
