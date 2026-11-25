//! Several jobs laid on one plate, checked together and written as one printer file, each
//! prepared in its own job first (ADR 0478).

use anyhow::{Result, bail};
use serde_json::json;

use super::reply::Reply;
use super::{Context, Server};
use crate::jobs::Revision;
use crate::params::{PlateJobs, absolute};

/// Why jobs on different machines are not laid on one plate.
const MACHINES_DIFFER: &str = "the jobs name different printers or resins; one plate prints on \
    one machine, so set them alike first";

/// Why a job on the standard printer and stock resin is not laid on a plate to print.
const UNNAMED_MACHINE: &str = "a job runs on the standard printer and stock resin; ask the user \
    which printer and resin they print on and set them on every job first";

impl Server {
    /// Lays each job's latest revision on one plate, spread apart, and checks it; writes it to
    /// `output` where one is given, unless the check names defects the user did not accept.
    pub(super) async fn laid_together(&self, laid: PlateJobs, ctx: &Context) -> Result<Reply> {
        if laid.jobs.len() < 2 {
            bail!("name two jobs or more; write_file writes one");
        }
        let revisions = (laid.jobs.iter())
            .map(|job| self.jobs.open(job, None))
            .collect::<Result<Vec<Revision>>>()?;
        let first = &revisions[0].recipe;
        if !first.names_a_machine() {
            bail!("{UNNAMED_MACHINE}");
        }
        let alike = |revision: &Revision| {
            revision.recipe.printer == first.printer && revision.recipe.resin == first.resin
        };
        if !revisions.iter().all(alike) {
            bail!("{MACHINES_DIFFER}");
        }
        let mut args = vec![
            if laid.output.is_some() {
                "slice"
            } else {
                "estimate"
            }
            .to_owned(),
        ];
        for revision in &revisions {
            args.push(self.jobs.plate_of(revision)?.display().to_string());
        }
        args.push("--arrange".into());
        args.extend(first.machine_flags());
        if let Some(output) = &laid.output {
            absolute(output, "the output")?;
            args.push(format!("--output={}", output.display()));
            if !laid.accept_defects {
                args.push("--strict".into());
            }
        }
        let outcome = self.watched(&args, ctx).await?;
        let done = outcome.exit_code == 0;
        let laid_out: Vec<_> = (revisions.iter())
            .map(|revision| json!({"job": revision.job, "revision": revision.number}))
            .collect();
        let body = json!({
            "jobs": laid_out,
            "written": laid.output.is_some() && done,
            "output": laid.output.filter(|_| done).map(|path| path.display().to_string()),
            "why_not": (!done).then_some(if outcome.reported() {
                "the check names defects; nothing was written"
            } else {
                "the command line refused the plate"
            }),
            "report": outcome.report,
        });
        Ok(Reply {
            body,
            failed: !done,
        })
    }
}
