//! `cargo xtask web`: the window built for a browser, with threads, ready to serve. What
//! each step is for is in `docs/design/web-build.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(clap::Args)]
pub struct Args {
    /// The nightly toolchain to build with: threads in wasm rebuild the standard library.
    #[arg(long, default_value = "nightly")]
    toolchain: String,
    /// Where the page, its script and its module are put.
    #[arg(long, default_value = "target/web/dist")]
    out: PathBuf,
    /// Skips `wasm-opt`, for a build that only has to run.
    #[arg(long)]
    no_opt: bool,
    /// Only compiles, which is what CI needs: no bindings, no page.
    #[arg(long)]
    check: bool,
}

/// Every thread is a worker on one shared memory, which the module imports rather than
/// owns, with room for wasm32's whole 4 GiB; the TLS exports are what a worker sets its
/// thread-locals up with.
const RUSTFLAGS: &str = concat!(
    "--cfg getrandom_backend=\"wasm_js\" ",
    "-C target-feature=+atomics,+bulk-memory,+mutable-globals ",
    "-C link-arg=--shared-memory ",
    "-C link-arg=--import-memory ",
    "-C link-arg=--max-memory=4294967296 ",
    "-C link-arg=--export=__wasm_init_tls ",
    "-C link-arg=--export=__tls_size ",
    "-C link-arg=--export=__tls_align ",
    "-C link-arg=--export=__tls_base",
);

/// Kept apart from the single-threaded builds, whose flags would otherwise rebuild it all.
const TARGET_DIR: &str = "target/web";

const WASM: &str = "target/web/wasm32-unknown-unknown/release/encrust_web.wasm";

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root()?;
    let pkg = root.join(&args.out);

    step(
        Command::new("cargo")
            .current_dir(&root)
            .arg(format!("+{}", args.toolchain))
            .arg(if args.check { "check" } else { "build" })
            .args(["--release", "--locked", "-p", "encrust-web"])
            .args(["--target", "wasm32-unknown-unknown"])
            .args(["-Z", "build-std=panic_abort,std"])
            .env("CARGO_TARGET_DIR", TARGET_DIR)
            .env("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS", RUSTFLAGS),
        "cargo build",
    )?;
    if args.check {
        return Ok(());
    }
    step(
        Command::new("wasm-bindgen")
            .current_dir(&root)
            .args(["--target", "web", "--out-dir"])
            .arg(&pkg)
            .arg(WASM),
        "wasm-bindgen",
    )?;
    if !args.no_opt {
        let module = pkg.join("encrust_web_bg.wasm");
        step(
            Command::new("wasm-opt")
                .arg("-O3")
                .args([
                    "--enable-threads",
                    "--enable-bulk-memory",
                    "--enable-nontrapping-float-to-int",
                    "--enable-sign-ext",
                    "--enable-mutable-globals",
                    "--enable-reference-types",
                    "--enable-multivalue",
                ])
                .arg(&module)
                .arg("-o")
                .arg(&module),
            "wasm-opt",
        )?;
    }
    copy_page(&root.join("crates/encrust-web/www"), &pkg)?;
    println!("{}", pkg.display());
    Ok(())
}

fn step(command: &mut Command, name: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("cannot run {name}; see docs/design/web-build.md"))?;
    if !status.success() {
        bail!("{name} failed with {status}");
    }
    Ok(())
}

fn copy_page(from: &Path, to: &Path) -> Result<()> {
    for entry in
        std::fs::read_dir(from).with_context(|| format!("cannot read {}", from.display()))?
    {
        let path = entry?.path();
        if path.is_file() {
            let name = path.file_name().unwrap_or_default();
            std::fs::copy(&path, to.join(name))
                .with_context(|| format!("cannot copy {}", path.display()))?;
        }
    }
    Ok(())
}

/// `cargo xtask` runs from wherever it was typed; the paths here are the workspace's.
fn workspace_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .context("xtask is not inside the workspace")
}
