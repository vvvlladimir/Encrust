//! Where the models of a plate made here stand: spread over it, put at a spot, and the plugs
//! for their holes kept clear of the plate's edge and of each other.

use std::borrow::Cow;
use std::path::Path;

use anyhow::{Context, Result, bail};
use core_geometry::{Aabb, Mesh, Transform, Vec2, transform_mesh};
use core_plate::{
    ArrangeSettings, Body, BodyKind, Footprint, PlateArea, Rect, arrange, crossings, move_clear,
    turned_onto,
};
use printer_profiles::PrinterProfile;

/// The model moved so the middle of its footprint is over `position` and its lowest
/// point is on the plate.
pub(super) fn stood_at(mesh: &Mesh, position: Vec2) -> Result<Mesh> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    let middle = (bounds.mins.truncate() + bounds.maxs.truncate()) / 2.0;
    let offset = (position - middle).extend(-bounds.mins.z);
    Ok(transform_mesh(mesh, Transform::from_translation(offset)))
}

/// What of `printer`'s plate its inset leaves.
fn plate_area(printer: &PrinterProfile) -> PlateArea {
    let [low, high] = printer.usable_area_mm();
    PlateArea {
        low_mm: Vec2::from_array(low),
        high_mm: Vec2::from_array(high),
    }
}

/// How each of `placed`, standing where it prints, moves so that only what is past the usable
/// area or meets another moves, the least it takes, else as [`arranged`] packs it (ADR 0418).
pub(crate) fn cleared(
    placed: &[(&Mesh, &Path)],
    printer: Option<&PrinterProfile>,
    settings: &ArrangeSettings,
) -> Result<Vec<Transform>> {
    let printer = printer.context("moving models clear needs a printer to know the plate")?;
    let area = plate_area(printer);
    let astray = astray_of(placed, area);
    let turns = turns_onto(placed, area, settings, &astray);
    let turned = turned_meshes(placed, &turns);
    let parts = turned
        .iter()
        .zip(placed)
        .map(|(mesh, (_, path))| {
            let bounds = mesh
                .aabb()
                .with_context(|| format!("{} has no vertices", path.display()))?;
            Ok(Rect::under(&bounds))
        })
        .collect::<Result<Vec<_>>>()?;
    let cleared = move_clear(&parts, &astray, &meeting_of(&turned), area, settings);
    if !cleared.left_out.is_empty() {
        return arranged(placed, Some(printer), settings);
    }
    let mut moves = turns;
    for spot in &cleared.moved {
        moves[spot.index].translation += spot.offset_mm.extend(0.0);
    }
    Ok(moves)
}

/// The indices of `placed` reaching past `area`.
fn astray_of(placed: &[(&Mesh, &Path)], area: PlateArea) -> Vec<usize> {
    let past = |bounds: Aabb| {
        bounds.mins.truncate().cmplt(area.low_mm).any()
            || bounds.maxs.truncate().cmpgt(area.high_mm).any()
    };
    placed
        .iter()
        .enumerate()
        .filter(|(_, (mesh, _))| mesh.aabb().is_some_and(past))
        .map(|(index, _)| index)
        .collect()
}

/// Each pair of `meshes`, by index, that meet.
fn meeting_of(meshes: &[Cow<'_, Mesh>]) -> Vec<[usize; 2]> {
    let bodies: Vec<Body<'_>> = meshes
        .iter()
        .enumerate()
        .map(|(owner, mesh)| Body {
            owner,
            kind: BodyKind::Model,
            mesh,
            bvh: None,
            transform: Transform::default(),
        })
        .collect();
    crossings(&bodies)
        .iter()
        .map(|meeting| meeting.owners)
        .collect()
}

/// How each of `placed`, a mesh standing where it prints and the file it came from, moves
/// to pack the plate biggest first, a model no slide fits turned; refuses one that does not fit.
pub(crate) fn arranged(
    placed: &[(&Mesh, &Path)],
    printer: Option<&PrinterProfile>,
    settings: &ArrangeSettings,
) -> Result<Vec<Transform>> {
    let printer = printer.context("--arrange needs a printer to know the plate")?;
    let area = plate_area(printer);
    let every: Vec<usize> = (0..placed.len()).collect();
    let turns = turns_onto(placed, area, settings, &every);
    let footprints = placed
        .iter()
        .zip(&turns)
        .map(|((mesh, path), turn)| {
            Footprint::of(mesh, *turn, settings.cell_mm)
                .with_context(|| format!("{} has no footprint", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let arranged = arrange(&footprints, area, settings);
    if let Some(&first) = arranged.left_out.first() {
        bail!(
            "{} of {} models found no room on the plate, {} among them",
            arranged.left_out.len(),
            placed.len(),
            placed[first].1.display()
        );
    }
    let mut moves = turns;
    for spot in &arranged.placed {
        moves[spot.index].translation += spot.offset_mm.extend(0.0);
    }
    Ok(moves)
}

/// The turn about the vertical through its middle that brings each of `placed` named in
/// `only` onto `area` where no slide does, the least that does (ADR 0418); none for the rest.
fn turns_onto(
    placed: &[(&Mesh, &Path)],
    area: PlateArea,
    settings: &ArrangeSettings,
    only: &[usize],
) -> Vec<Transform> {
    let mut turns = vec![Transform::default(); placed.len()];
    for &index in only {
        let mesh = placed[index].0;
        let Some(bounds) = mesh.aabb() else {
            continue;
        };
        let rect = Rect::under(&bounds);
        let outline = || {
            mesh.vertices
                .iter()
                .map(|vertex| vertex.truncate())
                .collect()
        };
        if let Some(moved) = turned_onto(rect, outline, area, settings) {
            turns[index] = moved;
        }
    }
    turns
}

/// Each of `placed` as `turns` leaves it, borrowed where it was not turned.
fn turned_meshes<'a>(placed: &[(&'a Mesh, &Path)], turns: &[Transform]) -> Vec<Cow<'a, Mesh>> {
    placed
        .iter()
        .zip(turns)
        .map(|((mesh, _), turn)| match *turn == Transform::default() {
            true => Cow::Borrowed(*mesh),
            false => Cow::Owned(transform_mesh(mesh, *turn)),
        })
        .collect()
}
