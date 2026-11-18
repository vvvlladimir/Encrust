use super::standing::middle_of;
use super::{Pass, contact_of};
use crate::config::LEAN_BANDS;
use crate::detect::{across_mm, unsupported};
use crate::field::Field;
use crate::sample::{end_reach, sample_ends, sample_piece};
use core_geometry::Scalar;
use core_slicer::Layer;

/// What one layer wants held: the cells supports go at, each with its radius and what it is on,
/// and the specks that began on it, each with one cell and its area in mm2.
#[derive(Default)]
pub(super) struct Wanted {
    pub(super) cells: Vec<Want>,
    /// The ends of toes, each held unless a support stands within its own radius, not within
    /// the radius of whatever support stands near it.
    pub(super) ends: Vec<Want>,
    pub(super) specks: Vec<([i32; 2], Scalar)>,
}

/// One cell a layer wants held, the radius a support there holds, and what it is on.
pub(super) type Want = ([i32; 2], Scalar, On);

/// What a wanted cell is on.
#[derive(Debug, Clone, Copy)]
pub(super) enum On {
    /// An island of this many mm².
    Island(Scalar),
    Overhang,
    /// A step that put out this many mm³ in its layer.
    Step(Scalar),
    /// A face leaning past a step's angle and short of the profile's: held as a step
    /// where one below reaches it, else steadied on.
    Gentle,
    /// A slope a part is steadied on, sparsely.
    Slope,
}

impl Pass<'_> {
    /// What `fields[at]` wants held, where that layer is `index` of the stack and was cut
    /// at `cut`. `fields` holds it and the layers the lean is measured back over.
    pub(super) fn wanted(&self, fields: &[Field], at: usize, index: usize, cut: &Layer) -> Wanted {
        let layer = &fields[at];
        if layer.is_empty() || contact_of(cut, self.layer_height_mm) <= self.lowest_contact_mm {
            return Wanted::default();
        }

        // The first layers of a stack have nothing under them to be measured against.
        let seen = self.window.min(index).min(at);
        let nothing = Field::default();
        let (below, reference) = if seen == 0 {
            (&nothing, &nothing)
        } else {
            (&fields[at - 1], &fields[at - seen])
        };
        let rise_mm = seen as Scalar * self.layer_height_mm;

        let leans_deg = self
            .sizing
            .map(|auto| [auto.step_lean_deg, auto.steady_lean_deg]);
        let found = unsupported(
            layer,
            below,
            reference,
            self.grid,
            self.config,
            rise_mm,
            leans_deg,
        );
        if found.is_empty() {
            return Wanted::default();
        }
        let room = layer.shrunk(self.inset);
        let flatness = self.flatness(below, reference, rise_mm);
        // Islands first: a support under one also holds the overhang it is part of, which the
        // coverage check then leaves alone; steadied last, as sparsely as the profile says.
        let mut wanted = Wanted::default();
        self.want_islands(&found.islands, &room, &mut wanted);
        self.want_steps(&found.steps, &room, &mut wanted);
        self.want_overhangs(&found.overhangs, &room, &flatness, &mut wanted);
        self.want_level(&found.gentle, &room, self.spacing, On::Gentle, &mut wanted);
        self.want_slopes(&found.slopes, &room, &mut wanted);
        wanted.specks = self.specks_of(&found.specks);
        wanted
    }

    /// Each island's cells, held closer the smaller it is.
    pub(super) fn want_islands(&self, islands: &[Field], room: &Field, wanted: &mut Wanted) {
        for piece in islands {
            let area_mm2 = piece.cell_count() as Scalar * self.cell_mm2();
            let spacing_mm = self.config.island_spacing_mm(area_mm2);
            let coverage_mm = self.config.coverage_at(spacing_mm);
            let spacing = self.grid.cells_of(spacing_mm).max(1);
            let on = On::Island(area_mm2);
            wanted.cells.extend(
                sample_piece(piece, room, self.inset, spacing)
                    .into_iter()
                    .map(|cell| (cell, coverage_mm, on)),
            );
            self.want_ends(piece, room, spacing, on, wanted);
        }
    }

    /// The end of each toe of `piece`, wanted after the rest of it and held unless a support
    /// stands within [`end_reach`]: one at the toe's root holds its end only as a lever.
    pub(super) fn want_ends(
        &self,
        piece: &Field,
        room: &Field,
        spacing: i32,
        on: On,
        wanted: &mut Wanted,
    ) {
        let reach_mm = self.grid.millimetres_of(end_reach(spacing));
        wanted.ends.extend(
            sample_ends(piece, room, self.inset, spacing)
                .into_iter()
                .map(|cell| (cell, reach_mm, on)),
        );
    }

    /// Each overhang's cells, spaced by how flat it lies under each of them.
    pub(super) fn want_overhangs(
        &self,
        overhangs: &[Field],
        room: &Field,
        flatness: &Flatness<'_>,
        wanted: &mut Wanted,
    ) {
        for piece in overhangs {
            wanted.cells.extend(
                sample_piece(piece, room, self.inset, self.spacing)
                    .into_iter()
                    .map(|cell| {
                        let spacing_mm = self.config.band_spacing_mm(flatness.band_of(cell));
                        (cell, self.config.coverage_at(spacing_mm), On::Overhang)
                    }),
            );
            self.want_ends(piece, room, self.spacing, On::Overhang, wanted);
        }
    }

    /// Each step's cells; a step too small to be held as one is the overhang it always was.
    pub(super) fn want_steps(&self, steps: &[Field], room: &Field, wanted: &mut Wanted) {
        for piece in steps {
            let new_mm3 = piece.cell_count() as Scalar * self.cell_mm2() * self.layer_height_mm;
            let on = match self.sizing.and_then(|auto| auto.step_size(new_mm3)) {
                Some(_) => On::Step(new_mm3),
                None => On::Overhang,
            };
            self.want_level(std::slice::from_ref(piece), room, self.spacing, on, wanted);
        }
    }

    /// The slopes wide enough to steady a part on, held as sparsely as the profile says.
    pub(super) fn want_slopes(&self, slopes: &[Field], room: &Field, wanted: &mut Wanted) {
        let Some(auto) = self.sizing else {
            return;
        };
        let steady = self.grid.cells_of(auto.steady_spacing_mm).max(1);
        let least_mm = auto.steady_area_mm2.sqrt();
        for piece in slopes {
            if across_mm(piece, self.grid) >= least_mm {
                self.want_level(std::slice::from_ref(piece), room, steady, On::Slope, wanted);
            }
        }
    }

    /// The cells of `pieces` `spacing` cells apart, each held as a face looking down is.
    pub(super) fn want_level(
        &self,
        pieces: &[Field],
        room: &Field,
        spacing: i32,
        on: On,
        wanted: &mut Wanted,
    ) {
        let level_mm = self.config.coverage_at(self.config.spacing_mm);
        for piece in pieces {
            wanted.cells.extend(
                sample_piece(piece, room, self.inset, spacing)
                    .into_iter()
                    .map(|cell| (cell, level_mm, on)),
            );
            // A slope steadies the part; it hangs from nothing.
            if !matches!(on, On::Slope) {
                self.want_ends(piece, room, spacing, on, wanted);
            }
        }
    }

    /// Where each speck began, and its area in mm².
    pub(super) fn specks_of(&self, specks: &[Field]) -> Vec<([i32; 2], Scalar)> {
        specks
            .iter()
            .filter_map(|speck| {
                Some((
                    middle_of(speck)?,
                    speck.cell_count() as Scalar * self.cell_mm2(),
                ))
            })
            .collect()
    }

    /// The area of one cell of the grid, mm².
    pub(super) fn cell_mm2(&self) -> Scalar {
        self.grid.millimetres_of(1).powi(2)
    }
}

/// How flat an overhang lies, asked a cell at a time: only the cells sampled on it want
/// the answer, which is far fewer than the rows of the piece.
pub(super) struct Flatness<'f> {
    pub(super) leans: bool,
    pub(super) below: &'f Field,
    pub(super) reference: &'f Field,
    /// How far past `below` a cell is a shelf, in cells.
    pub(super) shelf: i32,
    /// How far past `reference` a cell is in each band past the first, in cells.
    pub(super) reaches: [i32; LEAN_BANDS - 1],
}

impl Flatness<'_> {
    /// The band `cell` lies in, 0 just past the angle and `LEAN_BANDS - 1` level.
    pub(super) fn band_of(&self, [x, y]: [i32; 2]) -> usize {
        if !self.leans {
            return LEAN_BANDS - 1;
        }
        if !self.below.reaches(x, y, self.shelf) {
            return LEAN_BANDS - 1;
        }
        self.reaches
            .iter()
            .take_while(|reach| !self.reference.reaches(x, y, **reach))
            .count()
    }
}
