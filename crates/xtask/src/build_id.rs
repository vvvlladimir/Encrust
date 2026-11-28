//! The build's id the binaries stamp their kept searches with, so the window and the command
//! line built from one tree share them (ADR 0466): the commit, and a hash of what is changed.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// The variable the engine reads the id from as it is compiled.
const BUILD_ID: &str = "ENCRUST_BUILD_ID";

/// Runs `cargo` with `args` in `root`, the tree's build id handed to the compiler.
pub(crate) fn cargo_build(root: &Path, args: &[&str]) -> Result<()> {
    let mut cargo = Command::new("cargo");
    cargo.current_dir(root).args(args);
    if std::env::var_os(BUILD_ID).is_none()
        && let Some(id) = tree_id(root)
    {
        cargo.env(BUILD_ID, id);
    }
    let status = cargo.status().context("cannot run cargo")?;
    if !status.success() {
        bail!("cargo {} failed with {status}", args.join(" "));
    }
    Ok(())
}

/// The commit checked out, and where anything differs from it the hash of the differences and
/// of every file git does not ignore yet; `None` outside a git tree.
fn tree_id(root: &Path) -> Option<String> {
    let head = git(root, &["rev-parse", "HEAD"])?;
    let head = String::from_utf8(head).ok()?.trim().to_owned();
    let mut changed = git(root, &["diff", "HEAD", "--binary"])?;
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    for path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        changed.extend_from_slice(path);
        changed.extend(std::fs::read(root.join(String::from_utf8_lossy(path).as_ref())).ok()?);
    }
    if changed.is_empty() {
        return Some(head);
    }
    let hashed = hash_object(root, &changed)?;
    Some(format!(
        "{}-{}",
        &head[..12.min(head.len())],
        &hashed[..12.min(hashed.len())]
    ))
}

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn hash_object(root: &Path, bytes: &[u8]) -> Option<String> {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(bytes).ok()?;
    let output = child.wait_with_output().ok()?;
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}
