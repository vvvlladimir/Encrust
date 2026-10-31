//! What a model is, read and checked without slicing it: `encrust inspect`.

use super::checked::checked;
use super::place::{centred, place};
use super::relief::press_relief;
use super::{Watch, load};
use crate::args::ImportArgs;
use crate::exit::Stop;
use crate::profiles::Chosen;
use crate::report::ImportReport;
use anyhow::Result;
use core_geometry::{Bvh, self_intersections, weld};
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

/// Loads and repairs a model without slicing it, for `inspect`, counting what `asked` names.
pub fn inspect(
    input: &Path,
    args: &ImportArgs,
    chosen: &Chosen,
    asked: Inspect,
) -> Result<ImportReport> {
    let profile = chosen.printer.as_ref();
    let loaded = load(input)?;
    let welded = weld(&loaded.mesh, args.weld_tolerance);
    let never = Stop::default();
    let quiet = Watch::quiet(&never);
    let (mesh, _) = place(welded.mesh.clone(), args, None, chosen, &quiet)?;
    // Pressed here too, so that what `inspect` measures is what would be printed.
    let mut mesh = press_relief(mesh, &welded, &loaded, args, false)?;
    if args.transform.center {
        mesh = centred(mesh, profile)?;
    }
    let mut report = checked(&mut mesh, input, welded, args, profile, asked.shells)?;
    if asked.intersections {
        report.crossing_pairs = Some(self_intersections(&mesh, &Bvh::build(&mesh)).len());
    }
    Ok(report)
}
