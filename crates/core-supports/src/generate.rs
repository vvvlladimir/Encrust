use core_geometry::{Scalar, Vec3};
use core_slicer::{Layer, Sliced};
use printer_profiles::{AutoProfile, SupportProfile};
use rayon::prelude::*;

use crate::column::MIN_PILLAR_HEIGHT_MM;
use crate::config::{
    GRID_PITCH_MM, LEAN_BANDS, MICRO_ISLAND_HEIGHT_MM, MICRO_ISLAND_MM3, MIN_ISLAND_MM,
    PLATE_CONTACT_MM, REFERENCE_RISE_MM, SampleConfig,
};
use crate::detect::across_mm;
use crate::field::{Field, Grid, Parts};
use crate::islands::{grid_of, hold_islands};
use crate::load::{Load, Mark};
use crate::need::{Need, Reason};
use crate::region::Blocked;
use crate::sample::{sample_piece, sample_stance};
use crate::step::Zones;

mod held;
mod standing;
mod wanted;
use standing::Standing;
use wanted::{Flatness, On, Wanted};

/// How many supports a part stands steady on: fewer hold it and let it rock on the peel.
const STEADY: u8 = 3;

/// How many layers are read and looked at in one go. Enough to keep every core busy, few
/// enough that a cancelled run stops inside a frame; see `docs/decisions/0033`.
const READ_BLOCK: usize = 32;

/// Where automatic placement wants supports under `stack`, in plate coordinates, around
/// the contacts already standing at `seeds`; see `docs/design/supports.md`.
#[allow(clippy::too_many_arguments)]
pub fn generate_supports(
    stack: &Sliced,
    layer_height_mm: Scalar,
    profile: &SupportProfile,
    sizing: Option<&AutoProfile>,
    seeds: &[Vec3],
    lows: &[Vec3],
    blocked: Option<&Blocked>,
    progress: &mut dyn FnMut(usize) -> bool,
) -> Vec<Need> {
    let Some(grid) = grid_of(stack, GRID_PITCH_MM) else {
        return Vec::new();
    };
    let config = SampleConfig::for_profile(profile);
    let pass = Pass::new(&grid, &config, profile, sizing, layer_height_mm, blocked);
    let mut run = Run::new(&pass, seeds, lows);
    if run.climb(stack, &pass, progress) {
        run.hold_islands(stack, &pass, seeds, lows);
    }
    needs(run.placed, &run.load)
}

impl<'a> Pass<'a> {
    fn new(
        grid: &'a Grid,
        config: &'a SampleConfig,
        profile: &SupportProfile,
        sizing: Option<&'a AutoProfile>,
        layer_height_mm: Scalar,
        blocked: Option<&'a Blocked>,
    ) -> Self {
        Self {
            grid,
            config,
            spacing: grid.cells_of(config.spacing_mm).max(1),
            inset: grid.cells_of(config.edge_inset_mm),
            window: ((REFERENCE_RISE_MM / layer_height_mm).round() as usize).max(1),
            layer_height_mm,
            keep_out_mm: profile.contact_radius_mm() + profile.clearance_mm,
            // Under a foot and the shortest pillar there is no room for a column, and
            // material that close to the plate does not sag before it cures.
            lowest_contact_mm: PLATE_CONTACT_MM
                .max(profile.footing_height_mm() + MIN_PILLAR_HEIGHT_MM),
            stance_mm: config.spacing_mm,
            blocked,
            sizing,
        }
    }
}

/// What a run has stood and read so far on its way up the stack.
struct Run {
    standing: Standing,
    load: Load,
    zones: Zones,
    steadied: Zones,
    placed: Vec<(Vec3, Why)>,
}

impl Run {
    fn new(pass: &Pass, seeds: &[Vec3], lows: &[Vec3]) -> Self {
        let (config, layer_height_mm) = (pass.config, pass.layer_height_mm);
        let tallest_mm = pass.sizing.map_or(0.0, |auto| {
            auto.step_height_mm.iter().copied().fold(0.0, Scalar::max)
        });
        let steady_mm = pass
            .sizing
            .map_or(0.0, |auto| config.coverage_at(auto.steady_spacing_mm));
        Self {
            standing: Standing::new(seeds, lows, pass.grid, config, layer_height_mm),
            load: Load::new(pass.grid.millimetres_of(1).powi(2) * layer_height_mm),
            zones: Zones::new(tallest_mm),
            steadied: Zones::new(steady_mm),
            placed: Vec::new(),
        }
    }

    /// Reads `stack` a block of layers at a time and puts down what each wants; answers
    /// `false` once `progress` has stopped the run.
    fn climb(
        &mut self,
        stack: &Sliced,
        pass: &Pass,
        progress: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        // The block being looked at, behind as much of the one before as the lean needs.
        let mut fields: Vec<Field> = Vec::new();
        let mut first_held = 0usize;
        for (block, layers) in stack.layers.chunks(READ_BLOCK).enumerate() {
            let fresh = fields.len();
            fields.par_extend(
                layers
                    .par_iter()
                    .map(|layer| Field::of_layer(layer, pass.grid)),
            );
            let looked: Vec<(Parts, Vec<u64>, Wanted)> = (fresh..fields.len())
                .into_par_iter()
                .map(|at| {
                    let parts = fields[at].parts();
                    let cells = parts.cells();
                    let index = first_held + at;
                    (
                        parts,
                        cells,
                        pass.wanted(&fields, at, index, &stack.layers[index]),
                    )
                })
                .collect();
            for (offset, (parts, cells, wanted)) in looked.into_iter().enumerate() {
                let index = block * READ_BLOCK + offset;
                if !progress(index) {
                    return false;
                }
                self.layer(pass, &stack.layers[index], parts, &cells, wanted);
            }
            let spent = fields.len() - pass.window.min(fields.len());
            fields.drain(..spent);
            first_held += spent;
        }
        true
    }

    /// Climbs onto one layer and puts down what it wants.
    fn layer(&mut self, pass: &Pass, layer: &Layer, parts: Parts, cells: &[u64], wanted: Wanted) {
        let contact_mm = contact_of(layer, pass.layer_height_mm);
        let parents = self.standing.parents_of(&parts);
        self.load.climb(parents.as_deref(), cells);
        self.standing.climb(&parts, parents, cells, contact_mm);
        self.zones.prune(contact_mm);
        self.steadied.prune(contact_mm);
        let placing = Placing {
            standing: &mut self.standing,
            load: &self.load,
            zones: &mut self.zones,
            steadied: &mut self.steadied,
            parts: &parts,
            contact_mm,
        };
        pass.put_down(placing, wanted, &mut self.placed);
        self.standing.rest_on(parts);
    }

    /// Moves contacts onto the islands of the whole stack and adds one where none reaches.
    fn hold_islands(&mut self, stack: &Sliced, pass: &Pass, seeds: &[Vec3], lows: &[Vec3]) {
        let mut contacts: Vec<Vec3> = self.placed.iter().map(|(contact, _)| *contact).collect();
        let skip = |at: Vec3| at.z <= pass.lowest_contact_mm || pass.refuses(at);
        let added = hold_islands(
            stack,
            pass.layer_height_mm,
            &mut contacts,
            seeds,
            lows,
            skip,
        );
        for ((contact, _), moved) in self.placed.iter_mut().zip(contacts) {
            *contact = moved;
        }
        let islands = added
            .into_iter()
            .map(|(at, area_mm2)| (at, Why::Island { area_mm2 }));
        self.placed.extend(islands);
    }
}

/// What was placed, with the model above each overhang contact now that all of it is known.
fn needs(placed: Vec<(Vec3, Why)>, load: &Load) -> Vec<Need> {
    let totals = load.totals();
    placed
        .into_iter()
        .map(|(contact, why)| Need {
            contact,
            reason: match why {
                Why::Island { area_mm2 } => Reason::Island { area_mm2 },
                Why::Overhang(mark) => Reason::Overhang {
                    carried_mm3: mark.map_or(0.0, |mark| mark.above_mm3(&totals)),
                },
                Why::Step { new_mm3 } => Reason::Step { new_mm3 },
                Why::Steady => Reason::Steady,
            },
        })
        .collect()
}

/// Why a contact was put down, while what stands above it is still being read.
#[derive(Debug, Clone, Copy)]
enum Why {
    Island { area_mm2: Scalar },
    Overhang(Option<Mark>),
    Step { new_mm3: Scalar },
    Steady,
}

/// What one layer is put down against: the supports standing so far, the graph of what
/// stands on what, the layer's pieces, and the height its contacts sit at.
struct Placing<'a> {
    standing: &'a mut Standing,
    load: &'a Load,
    /// The steps below that still reach this layer.
    zones: &'a mut Zones,
    /// The supports steadying a part on a slope that still keep the next one away.
    steadied: &'a mut Zones,
    parts: &'a Parts,
    contact_mm: Scalar,
}

/// Why a support goes on a cell that is `on`, touching at `contact`: what a step below
/// reaches is that step's, whatever leans there; `overhang` marks an overhang's load.
fn why_of(on: On, zones: &Zones, contact: Vec3, overhang: impl FnOnce() -> Option<Mark>) -> Why {
    let reached = match on {
        On::Overhang | On::Gentle => zones.reaching(contact),
        On::Island(_) | On::Step(_) | On::Slope => None,
    };
    match (on, reached) {
        (On::Island(area_mm2), _) => Why::Island { area_mm2 },
        (On::Step(new_mm3), _) | (On::Overhang | On::Gentle, Some(new_mm3)) => {
            Why::Step { new_mm3 }
        }
        (On::Overhang, None) => Why::Overhang(overhang()),
        (On::Gentle, None) | (On::Slope, _) => Why::Steady,
    }
}

/// Where a column put under `layer` touches, mm above the plate: a whole layer under the plane,
/// so a contact never stands on the surface it holds up; see `docs/design/supports.md`.
fn contact_of(layer: &Layer, layer_height_mm: Scalar) -> Scalar {
    layer.z - layer_height_mm
}

/// What one run measures with, so one layer can be looked at alone: what a layer needs held
/// depends only on the layers under it (ADR 0033).
struct Pass<'a> {
    grid: &'a Grid,
    config: &'a SampleConfig,
    /// How far apart supports go, in cells.
    spacing: i32,
    /// How far in from a layer's outline supports go, in cells.
    inset: i32,
    /// How many layers back the lean is measured over.
    window: usize,
    layer_height_mm: Scalar,
    /// How close to a blocked patch a contact may still be put.
    keep_out_mm: Scalar,
    lowest_contact_mm: Scalar,
    /// How far across a part has to be before the supports that steady it are put down.
    stance_mm: Scalar,
    blocked: Option<&'a Blocked>,
    /// The automatic profile steps are held and sized by, or `None` to hold them as
    /// overhangs.
    sizing: Option<&'a AutoProfile>,
}

impl Pass<'_> {
    /// How flat an overhang is, measured by how far each of its cells reaches past
    /// `reference`, `rise_mm` under it; a shelf grown in one layer over `below` is level.
    fn flatness<'f>(
        &self,
        below: &'f Field,
        reference: &'f Field,
        rise_mm: Scalar,
    ) -> Flatness<'f> {
        let mut reaches = [0; LEAN_BANDS - 1];
        for (band, reach) in (1..LEAN_BANDS).zip(&mut reaches) {
            *reach = self
                .grid
                .cells_of(rise_mm * self.config.band_lean_rad(band).tan());
        }
        Flatness {
            leans: self.config.spacing_leans(),
            below,
            reference,
            shelf: self.grid.cells_of(self.config.min_overhang_mm),
            reaches,
        }
    }
}

impl Pass<'_> {
    /// Puts down what one layer needs, in the order that makes a run repeatable: hanging points,
    /// loose pieces, what the layer `wanted`, then supports that steady a part.
    fn put_down(&self, at: Placing, wanted: Wanted, placed: &mut Vec<(Vec3, Why)>) {
        let Placing {
            standing,
            load,
            zones,
            steadied,
            parts,
            contact_mm,
        } = at;
        self.mark_specks(standing, parts, wanted.specks, contact_mm);
        self.hang_from_points(standing, contact_mm, placed);
        let anywhere = self.config.coverage_radius_mm;
        let adrift = self
            .adrift(standing, parts, contact_mm)
            .into_iter()
            .map(|(cell, area_mm2)| (cell, anywhere, On::Island(area_mm2)));
        let mut stepped = Vec::new();
        for (cell, coverage_mm, on) in adrift.chain(wanted.cells) {
            let at = self.grid.centre_mm(cell[0], cell[1]);
            let contact = at.extend(contact_mm);
            let piece = parts.at(cell[0], cell[1]);
            let why = why_of(on, zones, contact, || {
                piece.and_then(|piece| load.mark(piece))
            });
            let steadies = matches!(why, Why::Steady);
            if steadies && steadied.reaching(contact).is_some() {
                continue;
            }
            if let On::Step(new_mm3) = on {
                stepped.push((contact, new_mm3));
            }
            // Asked before the cell is taken, or a place nothing may stand on would
            // still count as held and leave its neighbours a support short.
            if self.refuses(contact) {
                standing.leave(piece);
                continue;
            }
            let taken = standing.take(piece, at, contact_mm, coverage_mm);
            if let Some(contact) = taken.filter(|_| steadies) {
                steadied.add(contact, self.steady_mm(), 0.0);
            }
            placed.extend(taken.map(|contact| (contact, why)));
        }
        self.reach_from_steps(zones, stepped);
        self.steady_parts(standing, parts, contact_mm, placed);
    }

    /// Notes where each speck began, or lets go of one the user painted supports out of.
    fn mark_specks(
        &self,
        standing: &mut Standing,
        parts: &Parts,
        specks: Vec<([i32; 2], Scalar)>,
        contact_mm: Scalar,
    ) {
        for (cell, area_mm2) in specks {
            let piece = parts.at(cell[0], cell[1]);
            let at = self.grid.centre_mm(cell[0], cell[1]).extend(contact_mm);
            if self.refuses(at) {
                standing.leave(piece);
            } else if let Some(piece) = piece {
                standing.speck(piece, at, area_mm2 * self.layer_height_mm);
            }
        }
    }

    /// Holds each part that begins at a point there, before it has any area to speak of.
    fn hang_from_points(
        &self,
        standing: &mut Standing,
        contact_mm: Scalar,
        placed: &mut Vec<(Vec3, Why)>,
    ) {
        let point = Why::Island { area_mm2: 0.0 };
        for (piece, tip) in self.hanging(standing, contact_mm) {
            if self.refuses(tip) {
                standing.leave(Some(piece));
            } else {
                placed.push((standing.hang(piece, tip), point));
            }
        }
    }

    /// Lets each step held on this layer reach the faces above it, as far as its size says.
    fn reach_from_steps(&self, zones: &mut Zones, stepped: Vec<(Vec3, Scalar)>) {
        let Some(auto) = self.sizing else {
            return;
        };
        for (at, new_mm3) in stepped {
            let size = auto.step_size(new_mm3).unwrap_or(0);
            zones.add(at, auto.step_height_mm[size], new_mm3);
        }
    }

    /// Props each part held at too few places, and lets it go.
    fn steady_parts(
        &self,
        standing: &mut Standing,
        parts: &Parts,
        contact_mm: Scalar,
        placed: &mut Vec<(Vec3, Why)>,
    ) {
        for (piece, cell) in self.unsteady(standing, parts, contact_mm) {
            let at = self.grid.centre_mm(cell[0], cell[1]);
            if !self.refuses(at.extend(contact_mm)) {
                let propped = standing.prop(piece, at, contact_mm, self.stance_mm / 4.0);
                placed.extend(propped.map(|contact| (contact, Why::Steady)));
            }
            standing.leave(Some(piece));
        }
    }

    /// How far one support steadying a part on a slope keeps the next away, mm.
    fn steady_mm(&self) -> Scalar {
        self.sizing
            .map_or(0.0, |auto| self.config.coverage_at(auto.steady_spacing_mm))
    }

    /// Whether the user has painted supports out of where `contact` would touch.
    fn refuses(&self, contact: Vec3) -> bool {
        self.blocked
            .is_some_and(|blocked| blocked.covers(contact, self.keep_out_mm))
    }

    /// The pieces nothing holds that began at a low point or as a speck and have grown past dust by
    /// the layer at `contact_mm`, each with its starting point: where its support goes.
    fn hanging(&self, standing: &Standing, contact_mm: Scalar) -> Vec<(u32, Vec3)> {
        standing
            .adrift()
            .filter_map(|piece| Some((piece, standing.hung(piece)?)))
            .filter(|(_, hung)| hung.point.z > self.lowest_contact_mm)
            .filter(|(_, hung)| {
                hung.volume_mm3 >= MICRO_ISLAND_MM3
                    && contact_mm - hung.point.z >= MICRO_ISLAND_HEIGHT_MM
            })
            .map(|(piece, hung)| (piece, hung.point))
            .collect()
    }

    /// The cells that hold up what `standing` has nothing under, each with its piece's area in mm2;
    /// a speck grown into an island is held or what grows from it prints in mid-air.
    fn adrift(
        &self,
        standing: &Standing,
        parts: &Parts,
        contact_mm: Scalar,
    ) -> Vec<([i32; 2], Scalar)> {
        if contact_mm <= self.lowest_contact_mm {
            return Vec::new();
        }
        let cell_mm2 = self.grid.millimetres_of(1).powi(2);
        standing
            .adrift()
            .map(|piece| parts.piece(piece))
            .filter(|piece| across_mm(piece, self.grid) >= MIN_ISLAND_MM)
            .flat_map(|piece| {
                let area_mm2 = piece.cell_count() as Scalar * cell_mm2;
                sample_piece(&piece, &piece.shrunk(self.inset), self.inset, self.spacing)
                    .into_iter()
                    .map(move |cell| (cell, area_mm2))
            })
            .collect()
    }

    /// Where the supports go that steady a part held at fewer than three places, with the
    /// piece each one is for: asked once the part is wide enough for them to stand apart.
    fn unsteady(
        &self,
        standing: &Standing,
        parts: &Parts,
        contact_mm: Scalar,
    ) -> Vec<(u32, [i32; 2])> {
        if contact_mm <= self.lowest_contact_mm {
            return Vec::new();
        }
        standing
            .unsteady()
            .into_iter()
            .map(|number| (number, parts.piece(number)))
            .filter(|(_, piece)| across_mm(piece, self.grid) >= self.stance_mm)
            .flat_map(|(number, piece)| {
                sample_stance(&piece, self.inset)
                    .into_iter()
                    .map(move |cell| (number, cell))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
