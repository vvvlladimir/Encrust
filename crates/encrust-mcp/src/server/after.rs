//! After the print: what the user saw, read against the check of the revision printed, and
//! kept with the job (`docs/design/print-failures.md`).

use anyhow::{Context as _, Result};
use serde_json::json;

use super::reply::Reply;
use super::{Context, Server};
use crate::params::ReportPrint;

impl Server {
    /// The likely causes of each symptom, the likeliest first, each with the change to try.
    pub(super) async fn after_print(&self, print: ReportPrint, ctx: &Context) -> Result<Reply> {
        let number = print.revision.or_else(|| self.jobs.written(&print.job));
        let revision = self.jobs.open(&print.job, number)?;
        let report = self
            .jobs
            .report_file(&revision.job, revision.number)
            .with_context(|| format!("revision {} keeps no check", revision.number))?;
        let mut args = vec!["diagnose".to_owned(), report.display().to_string()];
        for symptom in &print.symptoms {
            args.push(format!("--symptom={}", symptom.name()));
        }
        let outcome = self.watched(&args, ctx).await?;
        let kept = json!({
            "revision": revision.number,
            "symptoms": print.symptoms,
            "at_mm": print.at_mm,
            "note": print.note,
        });
        let prints = self.jobs.record_print(&revision.job, &kept)?;
        Ok(Reply {
            body: json!({
                "job": revision.job,
                "revision": revision.number,
                "prints_reported": prints,
                "at_mm": print.at_mm,
                "diagnosis": outcome.report,
            }),
            failed: outcome.exit_code != 0,
        })
    }
}
