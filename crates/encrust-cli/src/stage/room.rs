//! Where the models of a plate made here stand: spread over it, put at a spot, and the plugs
//! for their holes kept clear of the plate's edge and of each other.

use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::Model;
use core_geometry::{Mesh, Transform, Vec2, Vec3, transform_mesh};
use core_plate::{ArrangeSettings, Footprint, arrange};
use printer_profiles::PrinterProfile;

use super::{Shaping, bounds};

/// Refuses plugs that land off the plate or on a model: laid in a row past their own
/// model, they know nothing of the plate's edge or of its neighbours.
pub(super) fn check_plugs_clear(
    models: &[Model],
    plugged: &[(Range<usize>, &Path)],
    printer: Option<&PrinterProfile>,
) -> Result<()> {
    let footprints: Vec<Option<(Vec2, Vec2)>> = models.iter().map(footprint).collect();
    for (plugs, path) in plugged {
        for (mins, maxs) in footprints[plugs.clone()].iter().flatten() {
            let off = printer.is_some_and(|printer| {
                let plate = Vec2::new(printer.build_volume.x, printer.build_volume.y);
                mins.cmplt(Vec2::ZERO).any() || maxs.cmpgt(plate).any()
            });
            if off {
                bail!(
                    "the plugs for {} run off the plate; move the model left or leave out --drain-plugs",
                    path.display()
                );
            }
            let crowded = footprints.iter().enumerate().any(|(index, other)| {
                !plugs.contains(&index)
                    && other
                        .is_some_and(|(low, high)| mins.cmplt(high).all() && low.cmplt(*maxs).all())
            });
            if crowded {
                bail!(
                    "the plugs for {} land on another model; give it room on its right or leave out --drain-plugs",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

/// What `model` and its supports cover of the plate, its least and greatest corners.
fn footprint(model: &Model) -> Option<(Vec2, Vec2)> {
    bounds(model).map(|bounds| (bounds.mins.truncate(), bounds.maxs.truncate()))
}

/// Spreads every piece of every entry over `printer`'s plate.
pub(super) fn spread_pieces(
    pieces: &mut [Vec<Mesh>],
    entries: &[(PathBuf, Shaping)],
    printer: Option<&PrinterProfile>,
) -> Result<()> {
    let mut spread_out: Vec<(&mut Mesh, &Path)> = pieces
        .iter_mut()
        .zip(entries)
        .flat_map(|(meshes, (path, _))| meshes.iter_mut().map(move |mesh| (mesh, path.as_path())))
        .collect();
    spread(&mut spread_out, printer)
}

/// The model moved so the middle of its footprint is over `position` and its lowest
/// point is on the plate.
pub(super) fn stood_at(mesh: &Mesh, position: Vec2) -> Result<Mesh> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    let middle = (bounds.mins.truncate() + bounds.maxs.truncate()) / 2.0;
    let offset = (position - middle).extend(-bounds.mins.z);
    Ok(transform_mesh(mesh, Transform::from_translation(offset)))
}

/// Packs the models over the plate, biggest first, and refuses a plate they do not fit.
fn spread(placed: &mut [(&mut Mesh, &Path)], printer: Option<&PrinterProfile>) -> Result<()> {
    let footprints: Vec<(&Mesh, &Path)> =
        placed.iter().map(|(mesh, path)| (&**mesh, *path)).collect();
    let offsets = arranged(&footprints, printer, &ArrangeSettings::default())?;
    for ((mesh, _), offset) in placed.iter_mut().zip(offsets) {
        **mesh = transform_mesh(mesh, Transform::from_translation(offset));
    }
    Ok(())
}

/// How far each of `placed`, a mesh standing where it prints and the file it came from, moves
/// to pack the plate biggest first by `settings`; refuses a plate they do not fit.
pub(crate) fn arranged(
    placed: &[(&Mesh, &Path)],
    printer: Option<&PrinterProfile>,
    settings: &ArrangeSettings,
) -> Result<Vec<Vec3>> {
    let printer = printer.context("--arrange needs a printer to know the plate")?;
    let footprints = placed
        .iter()
        .map(|(mesh, path)| {
            Footprint::of(mesh, Transform::default(), settings.cell_mm)
                .with_context(|| format!("{} has no footprint", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let plate = Vec2::new(printer.build_volume.x, printer.build_volume.y);
    let arranged = arrange(&footprints, plate, settings);
    if let Some(&first) = arranged.left_out.first() {
        bail!(
            "{} of {} models found no room on the plate, {} among them",
            arranged.left_out.len(),
            placed.len(),
            placed[first].1.display()
        );
    }
    let mut offsets = vec![Vec3::ZERO; placed.len()];
    for spot in &arranged.placed {
        offsets[spot.index] = spot.offset_mm.extend(0.0);
    }
    Ok(offsets)
}
