//! `cargo xtask mcpb`: the MCP server and the command line it drives in one `.mcpb` bundle a
//! chat app installs, for one platform. See `docs/decisions/0455`.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::app::{path_arg, run_tool, workspace_root};

const MANIFEST: &str = include_str!("../../encrust-mcp/bundle/manifest.json");

/// The bundle for every Mac, both architectures in each binary, for a page that cannot tell
/// which Mac it is on.
const UNIVERSAL: &str = "universal-apple-darwin";
const MAC_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

/// The Encrust icon, shown beside the extension in the chat app.
const ICON: &[u8] = include_bytes!("../../../assets/icon/encrust-macos-512.png");

/// The binaries a bundle carries, by package.
const BINARIES: [(&str, &str); 2] = [("encrust-cli", "encrust"), ("encrust-mcp", "encrust-mcp")];

#[derive(clap::Args)]
pub struct Args {
    /// The target triple to build for, as the release builds, or `universal-apple-darwin` for
    /// both Mac architectures in one bundle; the host's when absent.
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
    let built = if target == UNIVERSAL {
        universal(&root)?
    } else {
        build(&root, args.target.as_deref())?
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

/// Builds both binaries for `target`, the host where none, and answers where they are.
fn build(root: &Path, target: Option<&str>) -> Result<PathBuf> {
    for (package, binary) in BINARIES {
        let mut asked = vec![
            "build",
            "--release",
            "--locked",
            "-p",
            package,
            "--bin",
            binary,
        ];
        asked.extend(
            target
                .map(|target| ["--target", target])
                .into_iter()
                .flatten(),
        );
        crate::build_id::cargo_build(root, &asked)?;
    }
    Ok(match target {
        Some(target) => root.join("target").join(target).join("release"),
        None => root.join("target/release"),
    })
}

/// Builds both Mac architectures and joins each binary into one that runs on either.
fn universal(root: &Path) -> Result<PathBuf> {
    let parts = MAC_TARGETS
        .iter()
        .map(|target| build(root, Some(target)))
        .collect::<Result<Vec<_>>>()?;
    let joined = root.join("target").join(UNIVERSAL).join("release");
    fs::create_dir_all(&joined)?;
    for (_, binary) in BINARIES {
        let output = joined.join(binary);
        let inputs: Vec<PathBuf> = parts.iter().map(|part| part.join(binary)).collect();
        let mut asked = vec!["-create", "-output", path_arg(&output)?];
        for input in &inputs {
            asked.push(path_arg(input)?);
        }
        run_tool(root, "lipo", &asked)?;
    }
    Ok(joined)
}

/// Writes the bundle: the manifest for `platform`, its icon and each binary under `server/`,
/// runnable.
fn pack(output: &Path, built: &Path, platform: &Platform) -> Result<()> {
    let file =
        File::create(output).with_context(|| format!("cannot write {}", output.display()))?;
    let mut zip = ZipWriter::new(file);
    zip.start_file("manifest.json", SimpleFileOptions::default())?;
    zip.write_all(manifest(platform).as_bytes())?;
    zip.start_file("icon.png", SimpleFileOptions::default())?;
    zip.write_all(ICON)?;
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

/// The manifest of a bundle for `platform` alone, its server named as that system runs it.
fn manifest(platform: &Platform) -> String {
    MANIFEST
        .replace("{{version}}", env!("CARGO_PKG_VERSION"))
        .replace("{{platform}}", platform.manifest)
        .replace("{{exe_suffix}}", platform.exe_suffix)
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
        assert_eq!(named(UNIVERSAL), "macos-universal darwin");
        assert!(Platform::of("wasm32-unknown-unknown").is_err());
    }

    #[test]
    fn a_windows_manifest_starts_the_exe_and_leaves_no_placeholder() {
        let windows = manifest(&Platform::of("aarch64-pc-windows-msvc").expect("a release target"));
        assert!(windows.contains(r#""entry_point": "server/encrust-mcp.exe""#));
        assert!(windows.contains(r#""command": "${__dirname}/server/encrust-mcp.exe""#));
        assert!(!windows.contains("{{"), "a placeholder left in:\n{windows}");
        let macos = manifest(&Platform::of("x86_64-apple-darwin").expect("a release target"));
        assert!(macos.contains(r#""command": "${__dirname}/server/encrust-mcp","#));
    }
}
