use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, covered_pieces};
use rayon::prelude::*;

use crate::cups::{CupWalk, Pieces};
use crate::error::PlateError;
use crate::orient::rotation_for;
use crate::printed::Printed;
use crate::section::Section;

/// What cutting a candidate through measured: what the second pass adds to its score.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cut {
    /// The largest cross-section, square millimetres.
    pub peak_mm2: Scalar,
    /// The hardest pull of any section on the film, newtons.
    pub pull_n: Scalar,
    /// The widest hole sealed from below, square millimetres, or zero for none.
    pub cup_mm2: Scalar,
}

/// How far apart a hollowed model is cut at most, millimetres, and the most sections that
/// may take: a cavity's floor is a wall thick, and a section has to land in it to seal it.
const SHELL_STEP_MM: Scalar = 1.0;
const SHELL_SECTIONS: usize = 128;

/// How far apart the planes through a cut stand at most, millimetres: under the thinnest
/// wall a hole goes through, so one of them meets the hole inside that wall.
const VENT_STEP_MM: Scalar = 0.5;

/// The most planes one run of cuts is given, however long a channel it is.
const VENT_PLANES: usize = 16;

/// Cuts the model standing with `down` at the plate at `sections` heights, and reads off
/// its largest section, its hardest pull and its widest suction cup.
///
/// The heights are samples across the model, not the layers it prints at: the hardest
/// section is a property of the shape. Each section is the pieces its contours cover, so
/// bodies that overlap there pull the film once (ADR 0206) and pieces apart pull apart.
pub fn cut(
    printed: &Printed,
    down: Vec3,
    sections: usize,
    on_plate: bool,
    min_cup_mm2: Scalar,
) -> Result<Cut, PlateError> {
    let turn = Transform {
        rotation: rotation_for(down),
        ..Transform::default()
    };
    let mut placed = transform_mesh(printed.shell.unwrap_or(printed.outer), turn);
    let bounds = placed.aabb().ok_or(PlateError::NothingToOrient)?;
    let (low, high) = (bounds.mins.z, bounds.maxs.z);
    if high - low <= Scalar::EPSILON {
        return Ok(Cut::default());
    }
    let sections = match printed.shell {
        Some(_) => (((high - low) / SHELL_STEP_MM).ceil() as usize).min(SHELL_SECTIONS),
        None => 0,
    }
    .max(sections)
    .max(1);
    let mut heights: Vec<Scalar> = (0..sections)
        .map(|index| low + (high - low) * (index as Scalar + 0.5) / sections as Scalar)
        .collect();
    if let Some(cuts) = printed.cuts {
        let cuts = transform_mesh(cuts, turn);
        heights.extend(vent_heights(&cuts, low, high));
        heights.sort_by(Scalar::total_cmp);
        append(&mut placed, &cuts);
    }

    let sliced = PlaneSliceEngine.slice_at(&placed, &heights)?;
    let stack: Vec<Pieces> = sliced
        .layers
        .par_iter()
        .map(|layer| covered_pieces(&layer.contours))
        .collect();

    let mut measured = Cut::default();
    let mut cups = CupWalk::new(on_plate, min_cup_mm2);
    for pieces in stack {
        let section = Section::of(&pieces);
        measured.peak_mm2 = measured.peak_mm2.max(section.area_mm2);
        measured.pull_n = measured.pull_n.max(section.pull_n);
        cups.push(pieces);
    }
    measured.cup_mm2 = cups.largest_mm2();
    Ok(measured)
}

/// Planes through every cut between `low` and `high`: a hole lets air into a cavity only
/// in a section that crosses it, and the even sections stand further apart than a hole
/// is wide. See `docs/design/orientation.md`.
fn vent_heights(cuts: &Mesh, low: Scalar, high: Scalar) -> Vec<Scalar> {
    let mut spans: Vec<(Scalar, Scalar)> = cuts
        .triangles()
        .map(|triangle| {
            let heights = [triangle.a.z, triangle.b.z, triangle.c.z];
            (
                heights.into_iter().fold(Scalar::INFINITY, Scalar::min),
                heights.into_iter().fold(Scalar::NEG_INFINITY, Scalar::max),
            )
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut runs: Vec<(Scalar, Scalar)> = Vec::new();
    for (from, to) in spans {
        match runs.last_mut() {
            Some(run) if from <= run.1 => run.1 = run.1.max(to),
            _ => runs.push((from, to)),
        }
    }
    runs.into_iter()
        .flat_map(|(from, to)| {
            let planes = (((to - from) / VENT_STEP_MM).ceil() as usize).clamp(1, VENT_PLANES);
            (0..planes)
                .map(move |index| from + (to - from) * (index as Scalar + 0.5) / planes as Scalar)
        })
        .filter(|height| *height > low && *height < high)
        .collect()
}

/// Lays `part` into `whole`, which the winding rule then subtracts where it is wound inward.
fn append(whole: &mut Mesh, part: &Mesh) {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}
