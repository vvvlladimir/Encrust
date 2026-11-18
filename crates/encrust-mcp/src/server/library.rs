//! The user's profiles and recipes: what there is to choose from, and a resin taken in.

use anyhow::{Context as _, Result, bail};

use super::reply::{Reply, reported, trim_profiles};
use super::{Context, Server};
use crate::params::{CreateResin, ImportResin, ListProfiles, SaveRecipe, absolute, plain};

impl Server {
    pub(super) async fn profiles(&self, list: ListProfiles, ctx: &Context) -> Result<Reply> {
        let outcome = self
            .watched(&["profiles".into(), "list".into()], ctx)
            .await?;
        Ok(reported(
            &outcome,
            trim_profiles(outcome.report.clone(), list.printer_query.as_deref()),
        ))
    }

    pub(super) async fn import(&self, import: ImportResin, ctx: &Context) -> Result<Reply> {
        absolute(&import.file, "the resin's file")?;
        let mut args = vec!["profiles".into(), "import".into()];
        let is_toml = import
            .file
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
        if is_toml {
            args.push("--kind=resin".into());
        }
        if let Some(printer) = &import.printer {
            plain(printer, "the printer")?;
            args.push(format!("--printer={printer}"));
        }
        args.extend(["--".into(), import.file.display().to_string()]);
        let outcome = self.watched(&args, ctx).await?;
        Ok(reported(&outcome, outcome.report.clone()))
    }

    /// Every recipe `recipes list` names, each that reads with its text from `recipes show`.
    pub(super) async fn recipes(&self, ctx: &Context) -> Result<Reply> {
        let listed = self
            .watched(&["recipes".into(), "list".into()], ctx)
            .await?;
        let mut report = listed.report.clone();
        if let Some(recipes) = report["recipes"].as_array_mut() {
            for recipe in recipes
                .iter_mut()
                .filter(|recipe| recipe["error"].is_null())
            {
                let Some(id) = recipe["id"].as_str().map(str::to_owned) else {
                    continue;
                };
                let args = ["recipes".into(), "show".into(), "--".into(), id];
                let shown = self.cli.run(&args).await;
                recipe["toml"] = shown.map_or(serde_json::Value::Null, |shown| {
                    shown.report["toml"].clone()
                });
            }
        }
        Ok(reported(&listed, report))
    }

    /// A resin of the user's from the numbers its maker states; the command line refuses one
    /// past what the printer takes, and names what it advises against.
    pub(super) async fn new_resin(&self, create: CreateResin, ctx: &Context) -> Result<Reply> {
        plain(&create.printer, "the printer")?;
        let mut args = vec!["profiles".to_owned(), "new-resin".to_owned()];
        args.extend(create.flags());
        let outcome = self.watched(&args, ctx).await?;
        Ok(reported(&outcome, outcome.report.clone()))
    }

    /// Keeps a job's steps, or a recipe the client wrote, as one of the user's recipes; the
    /// command line refuses one that would not run, and says why.
    pub(super) async fn keep_recipe(&self, save: SaveRecipe, ctx: &Context) -> Result<Reply> {
        let text = match (&save.job, save.toml) {
            (Some(job), None) => {
                let revision = self.jobs.open(job, save.revision)?;
                revision.steps.with_context(|| {
                    format!("revision {} of job {job} keeps no steps", revision.number)
                })?
            }
            (None, Some(toml)) => toml,
            _ => bail!("save_recipe takes a job or a recipe's toml, one of the two"),
        };
        let file = self.jobs.scratch_steps()?;
        std::fs::write(&file, text).with_context(|| format!("writing {}", file.display()))?;
        let args = [
            "recipes".into(),
            "save".into(),
            format!("--name={}", save.name),
            "--".into(),
            file.display().to_string(),
        ];
        let outcome = self.watched(&args, ctx).await;
        // A scratch file left behind is overwritten by no other run: its name is this run's.
        let _ = std::fs::remove_file(&file);
        let outcome = outcome?;
        Ok(reported(&outcome, outcome.report.clone()))
    }
}
