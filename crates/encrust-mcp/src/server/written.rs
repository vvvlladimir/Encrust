//! What leaves the job: the printer file written, the pictures drawn, the file sent.

use anyhow::{Result, bail};
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;

use super::reply::{Reply, answer, images, reported};
use super::revise::machine_of;
use super::{Context, MOST_MARKED, Server, findings};
use crate::jobs::Revision;
use crate::params::{Render, SendFile, View, WriteFile, absolute, plain};

/// Why a job on the standard printer and stock resin is not written yet.
const UNNAMED_MACHINE: &str = "the job runs on the standard printer and stock resin; ask the \
    user which printer they print on and which resin is in the vat, and pass both to \
    write_file";

impl Server {
    pub(super) async fn write(&self, write: WriteFile, ctx: &Context) -> Result<Reply> {
        absolute(&write.output, "the output")?;
        let mut revision = self.jobs.open(&write.job, write.revision)?;
        let (printer, resin) = machine_of(write.printer, write.resin)?;
        if !printer.is_empty() {
            (revision.recipe.printer, revision.recipe.resin) = (printer, resin);
        } else if !revision.recipe.names_a_machine() {
            bail!("{UNNAMED_MACHINE}");
        }
        let mut args = vec!["slice".to_owned()];
        args.extend(self.staged(&revision));
        args.push(format!("--output={}", write.output.display()));
        if !write.accept_defects {
            args.push("--strict".into());
        }
        let outcome = self.watched(&args, ctx).await?;
        let written = outcome.exit_code == 0;
        if written {
            self.jobs.mark_written(&revision.job, revision.number)?;
        }
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
    pub(super) async fn pictures(&self, render: Render, ctx: &Context) -> Result<CallToolResult> {
        let revision = self.jobs.open(&render.job, render.revision)?;
        let views = render
            .views
            .unwrap_or_else(|| vec![View::Iso, View::Bottom]);
        if views.is_empty() {
            bail!("name at least one view");
        }
        let size = render.size_px.unwrap_or(512).clamp(64, 1024);
        let marked = if render.mark_findings.unwrap_or(true) {
            let mut args = vec!["estimate".to_owned()];
            args.extend(self.staged(&revision));
            findings(&self.watched(&args, ctx).await?.report)
        } else {
            Vec::new()
        };
        let dir = self.jobs.pictures_dir(&revision)?;
        let names: Vec<&str> = views.iter().map(|view| view.name()).collect();
        let mut args = vec!["render".to_owned()];
        args.extend(self.staged(&revision));
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
        content.extend(images(&outcome.report).await?);
        Ok(CallToolResult::success(content))
    }

    /// The input and flags that stage `revision`: the plate it was made on, or the plate its
    /// runs left where its steps name objects, else its recipe.
    fn staged(&self, revision: &Revision) -> Vec<String> {
        let made = self.jobs.made_on_plate(revision);
        let left = (revision.recipe.scoped()).then(|| self.jobs.plate_of(revision).ok());
        let Some(plate) = made.or(left.flatten()) else {
            return revision.recipe.flags();
        };
        let mut flags = vec![plate.display().to_string()];
        flags.extend(revision.recipe.machine_flags());
        flags
    }

    pub(super) async fn send(&self, send: SendFile, ctx: &Context) -> Result<Reply> {
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
}
