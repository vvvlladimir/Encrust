//! Embeds every profile under `assets/profiles/` into the crate, so a fresh install has a
//! catalogue before it has any files on disk. See docs/decisions/0049. A kind with no
//! directory ships nothing, which is what a resin does: it is measured, not shipped
//! (docs/decisions/0196).
#![expect(
    clippy::expect_used,
    reason = "a build script reports failure by panicking"
)]

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/profiles");
    // Watching the root, not a kind that may be absent: cargo reruns a script every time one
    // of its watched paths does not exist, which rebuilds half the workspace on every call.
    println!("cargo:rerun-if-changed={}", assets.display());
    let mut source = String::new();
    emit(&mut source, "BUNDLED_PRINTERS", &assets.join("printers"));
    emit(&mut source, "BUNDLED_RESINS", &assets.join("resins"));
    emit(&mut source, "BUNDLED_SUPPORTS", &assets.join("supports"));

    let out = Path::new(&std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"))
        .join("bundled_profiles.rs");
    std::fs::write(&out, source).expect("the build directory is writable");
}

/// Writes one `&[(id, contents)]` table, sorted so the catalogue order is deterministic.
fn emit(source: &mut String, name: &str, dir: &Path) {
    if dir.is_dir() {
        println!("cargo:rerun-if-changed={}", dir.display());
    }

    let mut files: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|entry| entry.expect("a readable directory entry").path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "toml")
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("cannot read {}: {error}", dir.display()),
    };
    files.sort();

    writeln!(source, "const {name}: &[(&str, &str)] = &[").expect("writing to a String");
    for file in files {
        let id = file
            .file_stem()
            .expect("a .toml file has a stem")
            .to_str()
            .expect("profile file names are UTF-8");
        let path = file.to_str().expect("the assets path is UTF-8");
        println!("cargo:rerun-if-changed={path}");
        writeln!(source, "    ({id:?}, include_str!({path:?})),").expect("writing to a String");
    }
    writeln!(source, "];").expect("writing to a String");
}
