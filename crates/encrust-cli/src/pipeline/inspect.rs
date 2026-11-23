//! What a model is, read and checked without slicing it: `encrust inspect`.

use super::Watch;
use crate::args::{ImportArgs, ProfileArgs};
use crate::exit::Stop;
use crate::profiles::Chosen;
use crate::report::ImportReport;
use crate::stage::{Shaping, placed_by_edits};
use anyhow::{Context, Result};
use core_edit::ObjectId;
use core_geometry::{Bvh, self_intersections};
use core_mesh_io::Loaded;
use std::path::Path;

/// Says what a file carried beyond its triangles, when it carried anything.
pub(super) fn talk_mapping(loaded: &Loaded) {
    let Some(uvs) = &loaded.uvs else { return };
    let over = match uvs.mapped() {
        all if all == uvs.len() => format!("{all} faces"),
        some => format!("{some} of {} faces", uvs.len()),
    };
    let images: Vec<String> = loaded
        .textures
        .iter()
        .map(|texture| format!("{} ({} bytes)", texture.name, texture.bytes.len()))
        .collect();
    println!(
        "Textured: {} coordinates over {over}, from {}\n",
        uvs.mapped() * 3,
        images.join(", ")
    );
}

/// What `inspect` counts beyond what import always reports.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inspect {
    /// The pairs of faces that cross.
    pub intersections: bool,
    /// The shells, as import leaves the model, which is how `--drop-shells` numbers them.
    pub shells: bool,
}

/// A model read, mended, repaired and placed as `slice` takes it, without slicing it, for
/// `inspect`, counting what `asked` names.
pub fn inspect(
    input: &Path,
    args: &ImportArgs,
    (profile, chosen): (&ProfileArgs, &Chosen),
    asked: Inspect,
) -> Result<ImportReport> {
    let never = Stop::default();
    let quiet = Watch::quiet(&never);
    let entry = (input.to_path_buf(), Shaping::of_import(args.clone())?);
    let (staging, found) = placed_by_edits(&[entry], (profile, chosen), asked.shells, &quiet)?;
    let (mut report, _) = found.into_iter().next().context("one model was read")?;
    if asked.intersections {
        let mesh = staging.mesh(ObjectId(1)).unwrap_or_default();
        report.crossing_pairs = Some(self_intersections(&mesh, &Bvh::build(&mesh)).len());
    }
    Ok(report)
}
