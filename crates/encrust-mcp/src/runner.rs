//! Runs the `encrust` command line and reads the one JSON document `--json` promises
//! (`docs/cli.md`, "Output, exit codes and Ctrl-C").

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use tokio::process::Command;

/// The exit code `--strict` ends a run with when a model has defects or does not fit.
pub const DEFECTS: i32 = 3;

/// The `encrust` binary this server drives.
#[derive(Debug, Clone)]
pub struct Cli {
    program: PathBuf,
}

/// What one run of the command line said.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit_code: i32,
    pub report: Value,
}

impl Outcome {
    /// Whether the run ended in code 0.
    pub fn is_done(&self) -> bool {
        self.exit_code == 0
    }
}

impl Cli {
    pub fn new(program: PathBuf) -> Self {
        Self { program }
    }

    /// The `encrust` beside this server's own binary, as a bundle ships them, else the one on
    /// the `PATH`.
    pub fn beside(server: &Path) -> Self {
        let name = format!("encrust{}", std::env::consts::EXE_SUFFIX);
        let program = server
            .parent()
            .map(|dir| dir.join(&name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(name));
        Self::new(program)
    }

    /// Runs `encrust --json <args>`. A run that printed a report is an `Outcome` whatever its
    /// exit code; one that printed none is an error carrying what it wrote to stderr.
    pub async fn run(&self, args: &[String]) -> Result<Outcome> {
        let output = Command::new(&self.program)
            .arg("--json")
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
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
