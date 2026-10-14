//! `cargo xtask app`: the window as a macOS app bundle, under an id of its own, opened.
//! macOS lets a bundle that states why onto the local network; a binary started from a
//! terminal other than Terminal.app is never asked and never let on. See
//! `docs/decisions/0262`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// What the bundle is called and known by, apart from an installed release of the window,
/// so the Local Network list shows the two separately.
const NAME: &str = "Encrust Dev";
const IDENTIFIER: &str = "app.encrust.dev";

const INSTALL: &str = "cargo install cargo-packager --locked";

#[derive(clap::Args)]
pub struct Args {
    /// Builds with the release profile rather than the dev one.
    #[arg(long)]
    pub release: bool,
    /// Builds the bundle and leaves it closed.
    #[arg(long)]
    pub no_open: bool,
}

pub fn run(args: &Args) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("an app bundle is a macOS thing; run the window with `cargo run` here");
    }
    let root = workspace_root()?;
    let profile = if args.release { "release" } else { "debug" };
    build(&root, args.release)?;
    let packaged = root.join("target").join(profile).join("Encrust.app");
    let bundle = root
        .join("target")
        .join(profile)
        .join(format!("{NAME}.app"));
    if bundle.exists() {
        std::fs::remove_dir_all(&bundle)
            .with_context(|| format!("cannot remove {}", bundle.display()))?;
    }
    std::fs::rename(&packaged, &bundle)
        .with_context(|| format!("cannot move {} aside", packaged.display()))?;
    rename(&bundle)?;
    run_tool(
        &root,
        "codesign",
        &["--force", "--deep", "--sign", "-", path_arg(&bundle)?],
    )?;
    println!("{}", bundle.display());
    if !args.no_open {
        run_tool(&root, "open", &[path_arg(&bundle)?])?;
    }
    Ok(())
}

/// Both binaries the bundle carries, then the bundle around them. The packager builds
/// nothing itself; the release workflow builds first too.
fn build(root: &Path, release: bool) -> Result<()> {
    let profile: &[&str] = if release { &["--release"] } else { &[] };
    for (package, binary) in [("encrust-app", "encrust-gui"), ("encrust-cli", "encrust")] {
        let mut build = vec!["build", "-p", package, "--bin", binary];
        build.extend(profile);
        run_tool(root, "cargo", &build)?;
    }
    let mut package = vec!["packager", "-p", "encrust-app", "-f", "app"];
    package.extend(profile);
    run_tool(root, "cargo", &package).with_context(|| {
        format!("cannot package the window; install the packager with `{INSTALL}`")
    })
}

/// Gives the bundle its own name and id in its Info.plist, before it is signed.
fn rename(bundle: &Path) -> Result<()> {
    let plist = bundle.join("Contents/Info.plist");
    let plist = path_arg(&plist)?;
    for (key, value) in [
        ("CFBundleIdentifier", IDENTIFIER),
        ("CFBundleName", NAME),
        ("CFBundleDisplayName", NAME),
    ] {
        run_tool(
            bundle,
            "plutil",
            &["-replace", key, "-string", value, plist],
        )?;
    }
    Ok(())
}

fn run_tool(dir: &Path, tool: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(tool)
        .current_dir(dir)
        .args(args)
        .status()
        .with_context(|| format!("cannot run {tool}"))?;
    if !status.success() {
        bail!("{tool} {} failed with {status}", args.join(" "));
    }
    Ok(())
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("{} is not UTF-8", path.display()))
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .context("xtask is not inside the workspace")
}
