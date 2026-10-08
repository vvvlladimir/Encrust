//! `cargo xtask licenses`: the terms of every library compiled into a release, as one
//! markdown file. Why it is generated rather than written, and what ships it, is in
//! `docs/decisions/0214`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The generated file, at the root of the workspace. The release archive, the installers
/// and the browser build all take it from here.
pub const FILE: &str = "THIRD-PARTY-LICENSES.md";

/// What writes it, and what `about.toml` and `about.hbs` beside this file configure. The
/// `cli` feature is not default, and without it the install leaves no binary behind.
const INSTALL: &str = "cargo install cargo-about --locked --features cli";

#[derive(clap::Args)]
pub struct Args {
    /// Compares the file on disk with what this run would write, and fails if they differ.
    /// What CI runs: a dependency added without regenerating it is the drift to catch.
    #[arg(long)]
    pub check: bool,
}

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root()?;
    let path = root.join(FILE);
    let generated = generated(&root)?;

    if args.check {
        let found = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        if found != generated {
            bail!("{FILE} no longer matches the lockfile; run `cargo xtask licenses`");
        }
        println!("{} is current", path.display());
        return Ok(());
    }

    std::fs::write(&path, generated).with_context(|| format!("cannot write {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

/// The file as the lockfile says it should read. A crate under a licence `about.toml` does
/// not accept fails here, which is the point: its terms would otherwise reach a user
/// unstated.
fn generated(root: &Path) -> Result<String> {
    let run = Command::new("cargo")
        .current_dir(root)
        .args(["about", "generate", "--fail", "about.hbs"])
        .output()
        .with_context(|| format!("cannot run cargo-about; install it with `{INSTALL}`"))?;
    if !run.status.success() {
        let said = String::from_utf8_lossy(&run.stderr);
        bail!("cargo-about failed with {}:\n{}", run.status, said.trim());
    }
    String::from_utf8(run.stdout).context("cargo-about wrote something that is not UTF-8")
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .context("xtask is not inside the workspace")
}
