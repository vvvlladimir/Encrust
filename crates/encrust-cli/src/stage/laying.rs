//! The plate a run lays from its entries: each imported, mended, placed, modelled and
//! finished by the edits its flags fold to, then arranged (ADR 0472).

use std::path::PathBuf;

use anyhow::{Context, Result};
use core_document::Report;
use core_edit::{Edit, ObjectId};
use core_geometry::{Scalar, list_shells};
use printer_profiles::PrinterProfile;

use super::document::{Named, Staging, said};
use super::finished;
use super::{Found, Laid, Shaping, imported, modelled, placed, repaired};
use crate::args::{JobArgs, ProfileArgs};
use crate::pipeline::{Watch, read_again};
use crate::plate_edits::{EditArgs, each_edit};
use crate::profiles::Chosen;

/// Each entry imported, then mended, repaired, placed, pressed and modelled by the edits its
/// flags fold to; `by_recipe` leaves the orientation search and the repair to the recipe.
pub(super) fn shaped(
    entries: &[(PathBuf, Shaping)],
    (job, chosen): (&JobArgs, &Chosen),
    watch: &Watch,
    by_recipe: bool,
) -> Result<(Staging, Vec<Found>)> {
    // A recipe's orient and repair steps run what --orient and --repair ask; models combined
    // are joined where their files put them and the one they make is centred.
    let combining = job.model.combine.is_some();
    let edited: Vec<(PathBuf, Shaping)> = (entries.iter())
        .map(|(path, shaping)| {
            let mut shaping = shaping.clone();
            let transform = &mut shaping.import.transform;
            if by_recipe {
                transform.orient = false;
                shaping.import.repair = None;
            }
            if combining {
                transform.center = false;
            }
            (path.clone(), shaping)
        })
        .collect();
    let centred = entries
        .iter()
        .any(|(_, shaping)| shaping.import.transform.center);
    let printer = chosen.printer.as_ref();
    let pieces = job.model.makes_pieces();
    let (edited, late) = if pieces {
        placed::split_at_pieces(&edited)
    } else {
        (edited, Vec::new())
    };
    let (mut staging, mut found) = placed_by_edits(&edited, (&job.profile, chosen), false, watch)?;
    if watch.talk {
        for (import, _) in &found {
            print!("{import}");
        }
    }
    let centred = (combining && centred).then_some(printer);
    modelled::modelled(&mut staging, &job.model, centred, watch)?;
    if pieces {
        placed::placed(
            &mut staging,
            &mut found,
            &late,
            (&job.profile, printer),
            watch,
        )?;
    }
    Ok((staging, found))
}

/// Each entry imported, then mended, repaired, placed and pressed by its flags' edits, as `slice`
/// and `inspect` take it; `with_shells` lists its shells as import leaves it.
pub(crate) fn placed_by_edits(
    entries: &[(PathBuf, Shaping)],
    (profile, chosen): (&ProfileArgs, &Chosen),
    with_shells: bool,
    watch: &Watch,
) -> Result<(Staging, Vec<Found>)> {
    let (meshes, mut found) = imported(entries, chosen, watch)?;
    for (mesh, (import, _)) in meshes.iter().zip(&mut found) {
        import.shells = with_shells.then(|| list_shells(mesh));
    }
    let pieces = meshes.into_iter().map(|mesh| vec![mesh]).collect();
    let mut staging = Staging::of(pieces, &found, (entries, catalogue_ids(profile)));
    let printer = chosen.printer.as_ref();
    repaired::repaired(&mut staging, &mut found, entries, watch)?;
    placed::placed(&mut staging, &mut found, entries, (profile, printer), watch)?;
    repaired::pressed(&mut staging, entries, watch)?;
    read_again_where_reshaped(&staging, &mut found, entries, printer)?;
    Ok((staging, found))
}

/// The printer and resin the run names by catalogue id, which a project written keeps; none
/// for one of a file.
fn catalogue_ids(selection: &ProfileArgs) -> Named {
    let printer = selection
        .printer
        .clone()
        .filter(|_| selection.profile.is_none());
    let resin = selection
        .resin
        .clone()
        .filter(|_| selection.material.is_none());
    (printer, resin)
}

/// What import read off each entry's object, read again where an edit before modelling
/// changed it.
fn read_again_where_reshaped(
    staging: &Staging,
    found: &mut [Found],
    entries: &[(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
) -> Result<()> {
    for (entry, ((import, _), (_, shaping))) in found.iter_mut().zip(entries).enumerate() {
        let first = staging.targets(entry).first().copied();
        let mesh = first.and_then(|id| staging.mesh(id));
        if let Some(mesh) = mesh.filter(|_| repaired::reshaped(&shaping.import)) {
            read_again(import, &mesh, printer)?;
        }
    }
    Ok(())
}

/// Every entry's object hollowed, drilled, plugged and supported by the edits its flags fold
/// to, then the plate arranged where asked, last, with the supports; `held`: printer, drift mm.
pub(super) fn laid(
    mut staging: Staging,
    found: Vec<Found>,
    (entries, edits): (&[(PathBuf, Shaping)], &EditArgs),
    (held, arrange): ((Option<&PrinterProfile>, Scalar), bool),
    watch: &Watch,
) -> Result<Laid> {
    for (_, shaping) in entries {
        shaping.hollow.check_flags()?;
    }
    let finished = finished::finished(&mut staging, entries, held, watch)?;
    placed::copied(&mut staging, entries, watch)?;
    if arrange {
        arranged(&mut staging, entries, held.0, watch)?;
    }
    each_edit(&edits.listed()?, |edit| {
        let first = staging.ids().first().copied().unwrap_or(ObjectId::UNSET);
        let object = edit.object().unwrap_or(first);
        staging.apply(edit, object, watch.stop).map_err(said)?;
        Ok(())
    })?;
    let out = edits.project_out.as_deref();
    staging.laid((found, finished), (entries, out), held.0, watch.talk)
}

/// `--arrange`: every object spread over the usable plate with its supports, as the window's
/// Arrange packs it; one that finds no room is said and left where it stood.
pub(super) fn arranged(
    staging: &mut Staging,
    entries: &[(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
    watch: &Watch,
) -> Result<()> {
    let printer = printer.context("--arrange needs a printer to know the plate")?;
    let (low_mm, plate_mm) = finished::usable_area(printer);
    let edit = Edit::Arrange {
        low_mm,
        plate_mm,
        clearance_mm: core_edit::defaults::arrange_clearance_mm(),
    };
    let first = staging.ids().first().copied().unwrap_or(ObjectId::UNSET);
    let applied = staging
        .apply(&edit, first, watch.stop)
        .map_err(said)
        .context("arranging the plate")?;
    if let Report::Arranged { left_out, .. } = applied.report
        && let Some(&one) = left_out.first()
    {
        let named = staging
            .entry_of(one)
            .map(|entry| entries[entry].0.display().to_string());
        tracing::warn!(
            "{} of the plate's models found no room and stand where they were, {} among them",
            left_out.len(),
            named.unwrap_or_default()
        );
    }
    Ok(())
}
