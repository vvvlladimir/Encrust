use core_geometry::Scalar;

use crate::band::band;
use crate::join::{Halves, Planned, Work};
use crate::section::Section;
use crate::settings::Key;

/// Least resin the groove keeps outside it, mm, and the least a thin wall is let down to
/// before the tongue is narrowed to fit.
const WALL_MM: Scalar = 0.8;
const THIN_WALL_MM: Scalar = 0.4;

/// How far the tongue reaches back into its half, and the groove's cutter starts outside
/// its own, mm.
const SINK_MM: Scalar = 0.1;
const CLEAR_MM: Scalar = 0.5;

/// Groove depth past the tongue's end, mm, where spare glue goes.
const GLUE_MM: Scalar = 0.2;

/// Cells across the tongue, at least: a cell's centre reads the edge to within half a
/// cell, so twelve keep a 1.2 mm tongue's width to some 0.1 mm.
const CELLS_ACROSS: Scalar = 12.0;

/// The tongue's field and the groove's over `section`, positive where each stands, and how
/// many pieces of the face take a tongue.
fn fields(section: &Section, key: Key) -> (Vec<Scalar>, Vec<Scalar>, usize) {
    let clearance = key.clearance_mm;
    // Off every piece the fields stay below zero, as the distance itself is there.
    let mut tongue_field = section.distance.clone();
    let mut groove_field = section.distance.clone();
    let mut keyed = 0;
    let band_of = |distance: Scalar, from: Scalar, to: Scalar| (distance - from).min(to - distance);
    for cells in section.cells_by_piece() {
        let widest = cells
            .iter()
            .map(|cell| section.distance[*cell])
            .fold(0.0, Scalar::max);
        // A wall too thin for the tongue and both its walls keeps the tongue in its middle.
        let inset = (WALL_MM + clearance)
            .min(widest - key.width_mm / 2.0)
            .max(THIN_WALL_MM + clearance);
        for cell in &cells {
            let distance = section.distance[*cell];
            tongue_field[*cell] = band_of(distance, inset, inset + key.width_mm);
            groove_field[*cell] = band_of(
                distance,
                inset - clearance,
                inset + key.width_mm + clearance,
            );
        }
        keyed += usize::from(cells.iter().any(|cell| tongue_field[*cell] > 0.0));
    }
    (tongue_field, groove_field, keyed)
}

/// A tongue round the lower half's cut face and the groove in the upper it goes into.
pub(crate) fn key(halves: &Halves<'_>, key: Key) -> Planned {
    let finest = key.width_mm / CELLS_ACROSS;
    let Some(section) = Section::of(&halves.cut, halves.plane, finest) else {
        return Planned::default();
    };
    let clearance = key.clearance_mm;
    let (tongue_field, groove_field, keyed) = fields(&section, key);
    let tongue = band(&section, &tongue_field, -SINK_MM, key.height_mm);
    let groove = band(
        &section,
        &groove_field,
        -CLEAR_MM,
        key.height_mm + clearance + GLUE_MM,
    );
    Planned {
        below: Work {
            pegs: tongue,
            ..Work::default()
        },
        above: Work {
            sockets: groove,
            ..Work::default()
        },
        loose: Vec::new(),
        placed: keyed,
        bare: section.pieces - keyed,
    }
}
