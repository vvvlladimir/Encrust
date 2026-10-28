//! What import found in a model and what repair and the changes by hand did to it.

use core_geometry::Scalar;
use serde::Serialize;

use crate::report::ImportReport;

#[derive(Debug, Serialize)]
pub struct Model {
    pub vertices: usize,
    pub faces: usize,
    pub size_mm: [Scalar; 3],
    pub volume_mm3: Scalar,
}

#[derive(Debug, Serialize)]
pub struct Repair {
    pub vertices_merged: usize,
    pub faces_removed: usize,
    pub faces_flipped: usize,
    pub closed: bool,
    pub boundary_edges: usize,
    /// Loops of open edges, each a hole.
    pub holes: usize,
    pub shells: usize,
    /// Pairs of faces crossing each other, when `--intersections` asked for them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossing_pairs: Option<usize>,
    /// Holes `--repair` patched, and those it could not.
    pub holes_closed: usize,
    pub holes_left: usize,
    /// Open edges `--repair` split where the other side of a seam stood on them.
    pub seams_stitched: usize,
    /// Shells enclosing nothing `--repair` dropped as too small to keep.
    pub shells_dropped: usize,
    /// Whether `--repair=wrap` or `=deep` wrapped the model; absent when not asked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrapped: Option<bool>,
    /// `--repair` would have left more wrong, so the model is as it came.
    pub kept_as_found: bool,
    /// What the changes by hand did, when one was named.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_hand: Option<ByHand>,
    /// Each shell as import left the model, when `--shells` asked for them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_list: Option<Vec<ShellFacts>>,
}

/// What `--flip-faces`, `--drop-faces`, `--drop-shells` and `--bridge` did.
#[derive(Debug, Serialize)]
pub struct ByHand {
    pub faces_turned: usize,
    pub faces_added: usize,
    pub faces_dropped: usize,
    pub shells_dropped: usize,
}

/// One shell, numbered from 1 by its place in the list.
#[derive(Debug, Serialize)]
pub struct ShellFacts {
    pub faces: usize,
    pub area_mm2: Scalar,
    pub closed: bool,
}

#[derive(Debug, Serialize)]
pub struct Fit {
    pub printer: String,
    pub fits: bool,
    pub overflow_mm: [Scalar; 3],
    /// How far the model reaches past the envelope where it stands, per axis.
    pub outside_mm: [Scalar; 3],
    /// How far it reaches into the plate's inset, per axis; it still slices whole.
    pub inset_mm: [Scalar; 3],
}

pub(super) fn model_of(import: &ImportReport) -> Model {
    let size = import.stats.size();
    Model {
        vertices: import.stats.vertices,
        faces: import.stats.faces,
        size_mm: [size.x, size.y, size.z],
        volume_mm3: import.stats.volume,
    }
}

pub(super) fn repair_of(import: &ImportReport) -> Repair {
    let mended = import.mended.filter(|mended| !mended.kept_as_found);
    Repair {
        vertices_merged: import.welded.vertices_merged,
        faces_removed: import.welded.faces_removed()
            + import.doubled
            + mended.map_or(0, |mended| mended.duplicates_removed),
        faces_flipped: import
            .orientation
            .as_ref()
            .map_or(0, |found| found.flipped_faces),
        closed: import
            .diagnostics
            .as_ref()
            .is_some_and(core_geometry::MeshDiagnostics::is_closed),
        boundary_edges: import
            .diagnostics
            .as_ref()
            .map_or(0, |found| found.boundary_edges),
        holes: import.diagnostics.as_ref().map_or(0, |found| found.holes),
        shells: import.diagnostics.as_ref().map_or(0, |found| found.shells),
        crossing_pairs: import.crossing_pairs,
        holes_closed: mended.map_or(0, |mended| mended.filled.loops_filled),
        holes_left: mended.map_or(0, |mended| mended.filled.loops_left),
        seams_stitched: mended.map_or(0, |mended| mended.stitched.edges_split),
        shells_dropped: mended.map_or(0, |mended| mended.shells_dropped),
        wrapped: import
            .wrapped
            .map(|wrapped| wrapped != crate::report::Wrapped::LeftOut),
        kept_as_found: import.mended.is_some_and(|mended| mended.kept_as_found),
        by_hand: import.edited.map(|edited| ByHand {
            faces_turned: edited.faces_flipped,
            faces_added: edited.faces_added,
            faces_dropped: edited.faces_dropped,
            shells_dropped: edited.shells_dropped,
        }),
        shell_list: import.shells.as_ref().map(|shells| {
            shells
                .iter()
                .map(|shell| ShellFacts {
                    faces: shell.faces.len(),
                    area_mm2: shell.area_mm2,
                    closed: shell.closed,
                })
                .collect()
        }),
    }
}

pub(super) fn fit_of(import: &ImportReport) -> Option<Fit> {
    let fit = import.fit.as_ref()?;
    Some(Fit {
        printer: fit.printer.clone(),
        fits: fit.fits(),
        overflow_mm: [fit.overflow.x, fit.overflow.y, fit.overflow.z],
        outside_mm: [fit.outside.x, fit.outside.y, fit.outside.z],
        inset_mm: [fit.inset.x, fit.inset.y, fit.inset.z],
    })
}
