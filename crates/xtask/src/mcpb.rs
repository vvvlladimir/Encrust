//! `cargo xtask mcpb`: the MCP server and the command line it drives in one `.mcpb` bundle a
//! chat app installs, for one platform. See `docs/decisions/0455`.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::app::{run_tool, workspace_root};

const MANIFEST: &str = include_str!("../../encrust-mcp/bundle/manifest.json");

/// The binaries a bundle carries, by package.
const BINARIES: [(&str, &str); 2] = [("encrust-cli", "encrust"), ("encrust-mcp", "encrust-mcp")];

#[derive(clap::Args)]
pub struct Args {
    /// The target triple to build for, as the release builds; the host's when absent.
    #[arg(long)]
    pub target: Option<String>,
    /// Where the bundle is written; `target/mcpb/encrust-<os>-<arch>.mcpb` by default, the
    /// name a release attaches it under.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root()?;
    let target = args.target.clone().unwrap_or_else(host_triple);
    let platform = Platform::of(&target)?;
    for (package, binary) in BINARIES {
        let build = ["build", "--release", "-p", package, "--bin", binary];
        let targeted = ["--target", target.as_str()];
        let asked: Vec<&str> = build
            .into_iter()
            .chain(
                args.target
                    .is_some()
                    .then_some(targeted)
                    .into_iter()
                    .flatten(),
            )
            .collect();
        run_tool(&root, "cargo", &asked)?;
    }
    let built = match &args.target {
        Some(target) => root.join("target").join(target).join("release"),
        None => root.join("target/release"),
    };
    let output = args.output.clone().unwrap_or_else(|| {
        root.join(format!(
            "target/mcpb/encrust-{}-{}.mcpb",
            platform.os, platform.arch
        ))
    });
    let output = std::path::absolute(&output)?;
    if let Some(dir) = output.parent() {
        fs::create_dir_all(dir)?;
    }
    pack(&output, &built, &platform)?;
    println!("{}", output.display());
    Ok(())
}

/// Writes the bundle: the manifest for `platform` and each binary under `server/`, runnable.
fn pack(output: &Path, built: &Path, platform: &Platform) -> Result<()> {
    let file =
        File::create(output).with_context(|| format!("cannot write {}", output.display()))?;
    let mut zip = ZipWriter::new(file);
    let manifest = MANIFEST
        .replace("{{version}}", env!("CARGO_PKG_VERSION"))
        .replace("{{platform}}", platform.manifest);
    zip.start_file("manifest.json", SimpleFileOptions::default())?;
    zip.write_all(manifest.as_bytes())?;
    let runnable = SimpleFileOptions::default().unix_permissions(0o755);
    for (_, binary) in BINARIES {
        let name = format!("{binary}{}", platform.exe_suffix);
        let path = built.join(&name);
        let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
        zip.start_file(format!("server/{name}"), runnable)?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    Ok(())
}

/// A target as a bundle names it: the release's words for its system and architecture, and
/// the manifest's for its system.
struct Platform {
    os: &'static str,
    arch: String,
    manifest: &'static str,
    exe_suffix: &'static str,
}

impl Platform {
    fn of(triple: &str) -> Result<Self> {
        let arch = triple.split('-').next().unwrap_or_default().to_owned();
        let (os, manifest, exe_suffix) = if triple.contains("apple-darwin") {
            ("macos", "darwin", "")
        } else if triple.contains("windows") {
            ("windows", "win32", ".exe")
        } else if triple.contains("linux") {
            ("linux", "linux", "")
        } else {
            bail!("no .mcpb bundle is made for {triple}");
        };
        Ok(Self {
            os,
            arch,
            manifest,
            exe_suffix,
        })
    }
}

/// The triple this xtask was built for, which is the host's.
fn host_triple() -> String {
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => "unknown-linux-gnu",
    };
    format!("{}-{os}", std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_release_target_is_named_as_its_archive_is() {
        let named = |triple: &str| {
            let platform = Platform::of(triple).expect("a release target");
            format!("{}-{} {}", platform.os, platform.arch, platform.manifest)
        };
        assert_eq!(named("aarch64-apple-darwin"), "macos-aarch64 darwin");
        assert_eq!(named("x86_64-pc-windows-msvc"), "windows-x86_64 win32");
        assert_eq!(named("aarch64-unknown-linux-gnu"), "linux-aarch64 linux");
        assert!(Platform::of("wasm32-unknown-unknown").is_err());
    }
}
