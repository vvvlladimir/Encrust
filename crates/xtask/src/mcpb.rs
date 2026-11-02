//! `cargo xtask mcpb`: the MCP server and the command line it drives in one `.mcpb` bundle for
//! Claude Desktop, for the platform it is built on. See `docs/decisions/0396`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::app::{path_arg, run_tool, workspace_root};

const MANIFEST: &str = include_str!("../../encrust-mcp/bundle/manifest.json");

#[derive(clap::Args)]
pub struct Args {
    /// Where the bundle is written; `target/mcpb/encrust-<platform>.mcpb` by default.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root()?;
    for (package, binary) in [("encrust-cli", "encrust"), ("encrust-mcp", "encrust-mcp")] {
        run_tool(
            &root,
            "cargo",
            &["build", "--release", "-p", package, "--bin", binary],
        )?;
    }
    let platform = platform()?;
    let stage = root.join("target/mcpb/stage");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("cannot clear {}", stage.display()))?;
    }
    fs::create_dir_all(stage.join("server"))?;
    let manifest = MANIFEST
        .replace("{{version}}", env!("CARGO_PKG_VERSION"))
        .replace("{{platform}}", platform);
    fs::write(stage.join("manifest.json"), manifest)?;
    for binary in ["encrust", "encrust-mcp"] {
        let name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
        let built = root.join("target/release").join(&name);
        fs::copy(&built, stage.join("server").join(&name))
            .with_context(|| format!("cannot copy {}", built.display()))?;
    }
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| root.join(format!("target/mcpb/encrust-{platform}.mcpb")));
    let output = std::path::absolute(&output)?;
    if output.exists() {
        fs::remove_file(&output)?;
    }
    run_tool(&stage, "zip", &["-r", "-q", path_arg(&output)?, "."])
        .context("a bundle is a zip archive; install `zip`")?;
    println!("{}", output.display());
    Ok(())
}

/// The platform name the manifest's `compatibility.platforms` takes.
fn platform() -> Result<&'static str> {
    Ok(match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        "linux" => "linux",
        other => bail!("Claude Desktop runs on no {other}"),
    })
}
