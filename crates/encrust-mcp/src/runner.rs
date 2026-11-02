//! Runs the `encrust` command line and reads the one JSON document `--json` promises
//! (`docs/cli.md`, "Output, exit codes and Ctrl-C").

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use tokio::process::Command;

/// The exit code `--strict` ends a run with when a model has defects or does not fit.
pub const DEFECTS: i32 = 3;

/// The `encrust` binary this server drives, and how long one run may take.
#[derive(Debug, Clone)]
pub struct Cli {
    program: PathBuf,
    timeout: Duration,
}

/// What one run of the command line said.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit_code: i32,
    pub report: Value,
}

impl Outcome {
    /// Whether the run ended in code 0, or in defects it reported.
    pub fn reported(&self) -> bool {
        self.exit_code == 0 || self.exit_code == DEFECTS
    }
}

impl Cli {
    pub fn new(program: PathBuf, timeout: Duration) -> Self {
        Self { program, timeout }
    }

    /// The `encrust` beside the server's own binary, as a bundle ships them, else the one on
    /// the `PATH`.
    pub fn program_beside(server: &Path) -> PathBuf {
        let name = format!("encrust{}", std::env::consts::EXE_SUFFIX);
        server
            .parent()
            .map(|dir| dir.join(&name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(name))
    }

    /// Runs `encrust --json <args>`. A run that printed a report is an `Outcome` whatever its
    /// exit code; one that printed none, or ran past the time limit, is an error. Dropping the
    /// future kills the child.
    pub async fn run(&self, args: &[String]) -> Result<Outcome> {
        let child = Command::new(&self.program)
            .arg("--json")
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(self.timeout, child)
            .await
            .with_context(|| {
                format!(
                    "encrust ran past {} s and was stopped",
                    self.timeout.as_secs()
                )
            })?
            .with_context(|| format!("starting {}", self.program.display()))?;
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let Ok(report) = serde_json::from_str::<Value>(stdout.trim()) else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "encrust exited with code {exit_code} and no report: {}",
                stderr.trim()
            );
        };
        Ok(Outcome { exit_code, report })
    }
}
