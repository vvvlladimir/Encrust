use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::GlobalArgs;
use crate::diagnose::{Diagnosis, Symptom, diagnose};
use crate::exit::Exit;
use crate::json;

#[derive(clap::Args, Debug)]
pub struct DiagnoseCommand {
    /// The `--json` report of the `estimate` or `slice` that made the file printed.
    #[arg(value_name = "REPORT")]
    pub report: PathBuf,
    /// What the print showed; once for each.
    #[arg(long = "symptom", value_enum, required = true, value_name = "SYMPTOM")]
    pub symptoms: Vec<Symptom>,
}

impl DiagnoseCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        let text = std::fs::read_to_string(&self.report)
            .with_context(|| format!("cannot read {}", self.report.display()))?;
        let report = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a --json report", self.report.display()))?;
        let diagnosis = diagnose(&report, &self.symptoms)?;
        if global.json {
            json::print(&diagnosis)?;
        } else if !global.quiet {
            print!("{}", told(&diagnosis));
        }
        Ok(Exit::Success)
    }
}

/// The diagnosis as a person reads it: each symptom, then its causes, the likeliest first.
fn told(diagnosis: &Diagnosis) -> String {
    let mut text = String::new();
    for read in &diagnosis.symptoms {
        let _ = writeln!(text, "{}: {}", read.symptom, read.means);
        for (number, cause) in read.causes.iter().enumerate() {
            let step = serde_json::to_value(cause.step).unwrap_or_default();
            let _ = writeln!(
                text,
                "  {}. {} ({})",
                number + 1,
                cause.cause,
                step.as_str().unwrap_or_default()
            );
            if let Some(seen) = &cause.seen {
                let _ = writeln!(text, "     seen: {seen}");
            }
            let _ = writeln!(text, "     try: {}", cause.change);
        }
    }
    text
}
