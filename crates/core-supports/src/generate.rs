use core_geometry::{FastMap, Scalar, Vec2, Vec3};
use core_slicer::{Layer, Sliced};
use printer_profiles::{AutoProfile, SupportProfile};
use rayon::prelude::*;

use crate::column::MIN_PILLAR_HEIGHT_MM;
use crate::config::{
    GRID_PITCH_MM, LEAN_BANDS, MICRO_ISLAND_HEIGHT_MM, MICRO_ISLAND_MM3, MIN_ISLAND_MM,
    PLATE_CONTACT_MM, REFERENCE_RISE_MM, SampleConfig,
};
use crate::detect::{across_mm, unsupported};
use crate::field::{Field, Grid, Parts};
use crate::load::{Load, Mark};
use crate::low::Lows;
use crate::need::{Need, Reason};
use crate::region::Blocked;
use crate::sample::{sample_piece, sample_stance};
use crate::step::Zones;

/// How many supports a part stands steady on: fewer hold it and let it rock on the peel.
const STEADY: u8 = 3;

/// How many layers are read and looked at in one go. Enough to keep every core busy, few
/// enough that a cancelled run stops inside a frame; see `docs/decisions/0033`.
const READ_BLOCK: usize = 32;

/// Where automatic placement wants supports, in plate coordinates.
///
/// `stack` is the model alone, already cut in plate coordinates; `seeds` are the contacts
/// of supports that already exist, which hold up what is near them and are never moved or
/// returned. `lows` are the model's [`crate::low_points`]: a part that begins at one is
/// held at that point itself rather than where its first layers are wide enough to see. `progress` is called with each layer index before what was found there is
/// put down, and stops the run by returning `false`, which leaves whatever was found up
/// to that layer.
///
/// Nothing is put where `blocked` reaches: a painted patch keeps automatic placement off
/// it the way it keeps a hand-placed support off it; see `docs/decisions/0093`.
///
/// The returned contacts sit a layer under the surface that needs them, each with what it
/// holds there. Whether one can actually stand there is a question for [`crate::columns`].
///
/// `sizing` is the automatic profile a run sizes its supports by: with one, a step and the
/// faces just above it are held as steps rather than overhangs; see `docs/decisions/0326`.
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
    let Some(grid) = grid_of(stack) else {
        return Vec::new();
    };
    let config = SampleConfig::for_profile(profile);
    let pass = Pass {
        grid: &grid,
        config: &config,
        spacing: grid.cells_of(config.spacing_mm).max(1),
        inset: grid.cells_of(config.edge_inset_mm),
        // How many layers back the lean is measured over.
        window: ((REFERENCE_RISE_MM / layer_height_mm).round() as usize).max(1),
        layer_height_mm,
        keep_out_mm: profile.contact_radius_mm() + profile.clearance_mm,
        // A contact lower than a foot and the shortest pillar that can stand on it has
        // nowhere to put a column, and material that close to the plate is not going to
        // sag before it cures anyway.
        lowest_contact_mm: PLATE_CONTACT_MM.max(profile.footing_height_mm() + MIN_PILLAR_HEIGHT_MM),
        stance_mm: config.spacing_mm,
        blocked,
        sizing,
    };

    let mut standing = Standing::new(seeds, lows, &grid, &config, layer_height_mm);
    let mut load = Load::new(grid.millimetres_of(1).powi(2) * layer_height_mm);
    let tallest_mm = sizing.map_or(0.0, |auto| {
        auto.step_height_mm.iter().copied().fold(0.0, Scalar::max)
    });
    let mut zones = Zones::new(tallest_mm);
    let steady_mm = sizing.map_or(0.0, |auto| config.coverage_at(auto.steady_spacing_mm));
    let mut steadied = Zones::new(steady_mm);
    let mut placed = Vec::new();
    // The block being looked at, with as much of the block before it as the lean is
    // measured over still in front of it.
    let mut fields: Vec<Field> = Vec::new();
    let mut first_held = 0usize;

    for (block, layers) in stack.layers.chunks(READ_BLOCK).enumerate() {
        let fresh = fields.len();
        fields.par_extend(layers.par_iter().map(|layer| Field::of_layer(layer, &grid)));
        let looked: Vec<(Parts, Vec<u64>, Wanted)> = (fresh..fields.len())
            .into_par_iter()
            .map(|at| {
                let parts = fields[at].parts();
                let cells = parts.cells();
                let wanted =
                    pass.wanted(&fields, at, first_held + at, &stack.layers[first_held + at]);
                (parts, cells, wanted)
            })
            .collect();

        for (offset, (parts, cells, wanted)) in looked.into_iter().enumerate() {
            let index = block * READ_BLOCK + offset;
            if !progress(index) {
                return needs(placed, &load);
            }
            let contact_mm = contact_of(&stack.layers[index], layer_height_mm);
            let parents = standing.parents_of(&parts);
            load.climb(parents.as_deref(), &cells);
            standing.climb(&parts, parents, &cells, contact_mm);
            zones.prune(contact_mm);
            steadied.prune(contact_mm);
            let placing = Placing {
                standing: &mut standing,
                load: &load,
                zones: &mut zones,
                steadied: &mut steadied,
                parts: &parts,
                contact_mm,
            };
            pass.put_down(placing, wanted, &mut placed);
            standing.rest_on(parts);
        }

        let spent = fields.len() - pass.window.min(fields.len());
        fields.drain(..spent);
        first_held += spent;
    }

    needs(placed, &load)
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

/// Where a column put under `layer` touches, in millimetres above the plate.
///
/// A whole layer under the plane, not half of one: a cell this layer is the first to
/// cover has the model's own surface somewhere inside that rise, and a contact above it
/// would stand a column on the very surface it is meant to hold up. A tip that bites sinks
/// back into the model, and one that does not is put on it; see `docs/design/supports.md`.
fn contact_of(layer: &Layer, layer_height_mm: Scalar) -> Scalar {
    layer.z - layer_height_mm
}

/// What one layer wants held: the cells supports go at, each with the radius it then
/// holds and what it is on, and the specks that began on it, each with one cell and its
/// area in mm².
#[derive(Default)]
struct Wanted {
    cells: Vec<Want>,
    specks: Vec<([i32; 2], Scalar)>,
}

/// One cell a layer wants held, the radius a support there holds, and what it is on.
type Want = ([i32; 2], Scalar, On);

/// What a wanted cell is on.
#[derive(Debug, Clone, Copy)]
enum On {
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

/// What one run measures with, so that one layer can be looked at on its own.
///
/// Every layer of a block is read and examined at once, because what a layer needs holding
/// up depends only on the layers under it and not on where a support was put; see
/// `docs/decisions/0033`.
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
    /// What `fields[at]` wants held, where that layer is `index` of the stack and was cut
    /// at `cut`. `fields` holds it and the layers the lean is measured back over.
    fn wanted(&self, fields: &[Field], at: usize, index: usize, cut: &Layer) -> Wanted {
        let layer = &fields[at];
        if layer.is_empty() || contact_of(cut, self.layer_height_mm) <= self.lowest_contact_mm {
            return Wanted::default();
        }

        // The first layers of a stack have nothing under them to be measured against.
        let seen = self.window.min(index).min(at);
        let nothing = Field::default();
        let below = if seen == 0 { &nothing } else { &fields[at - 1] };
        let reference = if seen == 0 {
            &nothing
        } else {
            &fields[at - seen]
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
        let cell_mm2 = self.grid.millimetres_of(1).powi(2);
        // Islands first: a support put down under one also holds the overhang it is part
        // of, which the coverage check then leaves alone.
        let islands = found.islands.iter().flat_map(|piece| {
            let area_mm2 = piece.cell_count() as Scalar * cell_mm2;
            let spacing_mm = self.config.island_spacing_mm(area_mm2);
            let coverage_mm = self.config.coverage_at(spacing_mm);
            let spacing = self.grid.cells_of(spacing_mm).max(1);
            sample_piece(piece, &room, self.inset, spacing)
                .into_iter()
                .map(move |cell| (cell, coverage_mm, On::Island(area_mm2)))
        });
        let flatness = self.flatness(below, reference, rise_mm);
        let overhangs = found.overhangs.iter().flat_map(|piece| {
            sample_piece(piece, &room, self.inset, self.spacing)
                .into_iter()
                .map(|cell| {
                    let spacing_mm = self.config.band_spacing_mm(flatness.band_of(cell));
                    (cell, self.config.coverage_at(spacing_mm), On::Overhang)
                })
        });
        // A step too small to be held as one is the overhang it always was.
        let level_mm = self.config.coverage_at(self.config.spacing_mm);
        let steps = found.steps.iter().flat_map(|piece| {
            let new_mm3 = piece.cell_count() as Scalar * cell_mm2 * self.layer_height_mm;
            let on = match self.sizing.and_then(|auto| auto.step_size(new_mm3)) {
                Some(_) => On::Step(new_mm3),
                None => On::Overhang,
            };
            sample_piece(piece, &room, self.inset, self.spacing)
                .into_iter()
                .map(move |cell| (cell, level_mm, on))
        });
        let gentle = found.gentle.iter().flat_map(|piece| {
            sample_piece(piece, &room, self.inset, self.spacing)
                .into_iter()
                .map(move |cell| (cell, level_mm, On::Gentle))
        });
        // Steadied last, as sparsely as the profile says, on slopes worth it.
        let (steady, least_mm) = self.sizing.map_or((1, Scalar::INFINITY), |auto| {
            let steady = self.grid.cells_of(auto.steady_spacing_mm).max(1);
            (steady, auto.steady_area_mm2.sqrt())
        });
        let slopes = found
            .slopes
            .iter()
            .filter(|piece| across_mm(piece, self.grid) >= least_mm)
            .flat_map(|piece| {
                sample_piece(piece, &room, self.inset, steady)
                    .into_iter()
                    .map(move |cell| (cell, level_mm, On::Slope))
            });
        Wanted {
            cells: islands
                .chain(steps)
                .chain(overhangs)
                .chain(gentle)
                .chain(slopes)
                .collect(),
            specks: found
                .specks
                .iter()
                .filter_map(|speck| {
                    Some((middle_of(speck)?, speck.cell_count() as Scalar * cell_mm2))
                })
                .collect(),
        }
    }
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

/// How flat an overhang lies, asked a cell at a time: only the cells sampled on it want
/// the answer, which is far fewer than the rows of the piece.
struct Flatness<'f> {
    leans: bool,
    below: &'f Field,
    reference: &'f Field,
    /// How far past `below` a cell is a shelf, in cells.
    shelf: i32,
    /// How far past `reference` a cell is in each band past the first, in cells.
    reaches: [i32; LEAN_BANDS - 1],
}

impl Flatness<'_> {
    /// The band `cell` lies in, 0 just past the angle and `LEAN_BANDS - 1` level.
    fn band_of(&self, [x, y]: [i32; 2]) -> usize {
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

impl Pass<'_> {
    /// Puts down what one layer needs, in the order that makes a run repeatable: the
    /// points parts hang from, the pieces nothing holds, what the layer itself `wanted`,
    /// and last the supports that steady a part held at too few places.
    fn put_down(&self, at: Placing, wanted: Wanted, placed: &mut Vec<(Vec3, Why)>) {
        let Placing {
            standing,
            load,
            zones,
            steadied,
            parts,
            contact_mm,
        } = at;
        for (cell, area_mm2) in wanted.specks {
            let piece = parts.at(cell[0], cell[1]);
            let at = self.grid.centre_mm(cell[0], cell[1]).extend(contact_mm);
            if self.refuses(at) {
                standing.leave(piece);
            } else if let Some(piece) = piece {
                standing.speck(piece, at, area_mm2 * self.layer_height_mm);
            }
        }
        // A part that begins at a point is held there before it has any area to speak of.
        let point = Why::Island { area_mm2: 0.0 };
        for (piece, tip) in self.hanging(standing, contact_mm) {
            if self.refuses(tip) {
                standing.leave(Some(piece));
            } else {
                placed.push((standing.hang(piece, tip), point));
            }
        }
        let adrift = self.adrift(standing, parts, contact_mm);
        let anywhere = self.config.coverage_radius_mm;
        let cells = adrift
            .into_iter()
            .map(|(cell, area_mm2)| (cell, anywhere, On::Island(area_mm2)));
        let mut stepped = Vec::new();
        for (cell, coverage_mm, on) in cells.chain(wanted.cells) {
            let at = self.grid.centre_mm(cell[0], cell[1]);
            let piece = parts.at(cell[0], cell[1]);
            // What a step below reaches is that step's, whatever leans there.
            let reached = match on {
                On::Overhang | On::Gentle => zones.reaching(at.extend(contact_mm)),
                On::Island(_) | On::Step(_) | On::Slope => None,
            };
            let why = match (on, reached) {
                (On::Island(area_mm2), _) => Why::Island { area_mm2 },
                (On::Step(new_mm3), _) | (On::Overhang | On::Gentle, Some(new_mm3)) => {
                    Why::Step { new_mm3 }
                }
                (On::Overhang, None) => Why::Overhang(piece.and_then(|piece| load.mark(piece))),
                (On::Gentle, None) | (On::Slope, _) => Why::Steady,
            };
            let steadies = matches!(why, Why::Steady);
            if steadies && steadied.reaching(at.extend(contact_mm)).is_some() {
                continue;
            }
            if let On::Step(new_mm3) = on {
                stepped.push((at.extend(contact_mm), new_mm3));
            }
            // Asked before the cell is taken, or a place nothing may stand on would
            // still count as held and leave its neighbours a support short.
            if self.refuses(at.extend(contact_mm)) {
                standing.leave(piece);
                continue;
            }
            let taken = standing.take(piece, at, contact_mm, coverage_mm);
            if let Some(contact) = taken.filter(|_| steadies) {
                steadied.add(contact, self.steady_mm(), 0.0);
            }
            placed.extend(taken.map(|contact| (contact, why)));
        }
        if let Some(auto) = self.sizing {
            for (at, new_mm3) in stepped {
                let size = auto.step_size(new_mm3).unwrap_or(0);
                zones.add(at, auto.step_height_mm[size], new_mm3);
            }
        }
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

    /// The pieces nothing holds that began at a low point of the model or as a speck and
    /// have grown into more than dust by the layer whose contacts sit at `contact_mm`,
    /// each with the point it began at: where its support goes.
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

    /// The cells that hold up what `standing` has nothing under, each with the area of its
    /// piece in mm²: a piece too small to be an island when it began is held once it has
    /// grown into one, or everything that grows out of it prints in mid-air. See
    /// `docs/design/supports.md`.
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

/// The supports standing under each piece of the layer the run has reached.
///
/// A piece inherits what stood under the pieces it grew out of, so a support suppresses a
/// sample only where the two hold the same part of the model: a column passing unrelated
/// material no longer leaves it a support short. See `docs/decisions/0081`.
///
/// Each piece's points are kept in buckets of the plate one coverage radius across, so
/// asking whether a place is already held reads nine buckets rather than every support
/// placed so far.
struct Standing {
    /// One set per piece of the layer last climbed to, numbered as `Parts` numbers them.
    held: Vec<Held>,
    /// How many supports stand under each of those pieces or under anything it grew out
    /// of, counted no further than [`STEADY`]; a piece on the plate has that many. Kept
    /// apart from `held`, which forgets a support left far enough below.
    tips: Vec<u8>,
    /// Where each of those pieces began, at a low point or as a speck, while nothing
    /// holds it yet.
    hung: Vec<Option<Hung>>,
    /// The model's low points the climb has still to match to a piece.
    lows: Lows,
    /// The pieces of the layer below, so a piece can be followed onto what it grew from.
    below: Option<Parts>,
    /// Supports handed to the run that the climb has not reached yet, highest last.
    pending: Vec<(Vec3, [i32; 2])>,
    /// Supports the climb has reached whose piece has not appeared at their cell yet.
    waiting: Vec<(Vec3, [i32; 2])>,
    /// Side of one bucket, millimetres: the widest radius over which a support holds.
    radius_mm: Scalar,
    /// The radius a support holds over when nothing says otherwise, millimetres.
    reach_mm: Scalar,
    /// How far above a place a support may stand and still hold it. A contact is put a
    /// layer under the surface it holds and a seed sits on it, so the two disagree by
    /// exactly that much.
    above_mm: Scalar,
    /// The resin one cell of one layer holds, cubic millimetres.
    cell_mm3: Scalar,
}

/// Where a piece nothing holds began, and how much resin has grown out of it since.
#[derive(Debug, Clone, Copy)]
struct Hung {
    point: Vec3,
    volume_mm3: Scalar,
}

impl Standing {
    fn new(
        seeds: &[Vec3],
        lows: &[Vec3],
        grid: &Grid,
        config: &SampleConfig,
        layer_height_mm: Scalar,
    ) -> Self {
        let mut pending: Vec<(Vec3, [i32; 2])> = seeds
            .iter()
            .map(|seed| (*seed, grid.cell_of(seed.truncate())))
            .collect();
        pending.sort_by(|a, b| b.0.z.total_cmp(&a.0.z));

        Self {
            held: Vec::new(),
            tips: Vec::new(),
            hung: Vec::new(),
            lows: Lows::new(lows, grid),
            below: None,
            pending,
            waiting: Vec::new(),
            // The widest any support holds, so that one bucket either way finds every
            // support that holds a place.
            radius_mm: config
                .coverage_at(config.steep_spacing_mm)
                .max(config.coverage_radius_mm)
                .max(Scalar::MIN_POSITIVE),
            reach_mm: config.coverage_radius_mm,
            above_mm: layer_height_mm,
            cell_mm3: grid.millimetres_of(1).powi(2) * layer_height_mm,
        }
    }

    /// What each piece of the next layer up grows out of on the layer last climbed to, or
    /// `None` on the first layer.
    fn parents_of(&self, parts: &Parts) -> Option<Vec<Vec<u32>>> {
        self.below.as_ref().map(|below| parts.parents(below))
    }

    /// Carries what is standing onto the pieces of the next layer up, whose contacts sit
    /// at `z`, given their [`Standing::parents_of`] and how many cells each covers, and
    /// takes on the seeds that layer has reached. The layer is handed back
    /// through [`Standing::rest_on`] once its own samples have been asked about.
    ///
    /// A point further below than the coverage radius is dropped on the way: it can hold
    /// nothing on this layer or on any layer above it, which is what keeps the run's
    /// memory to the supports that still matter rather than to every one ever placed.
    fn climb(&mut self, parts: &Parts, parents: Option<Vec<Vec<u32>>>, cells: &[u64], z: Scalar) {
        let on_plate = if z <= PLATE_CONTACT_MM { STEADY } else { 0 };
        let Some(parents) = parents else {
            self.held = vec![Held::default(); parts.count()];
            self.tips = vec![on_plate; parts.count()];
            self.hung = vec![None; parts.count()];
            self.take_seeds(parts, z);
            self.take_lows(parts, z);
            return;
        };
        self.hung = parents
            .iter()
            .map(|grown_from| {
                let mut hung = grown_from
                    .iter()
                    .filter_map(|parent| self.hung[*parent as usize]);
                let first = hung.next()?;
                let volume_mm3 =
                    first.volume_mm3 + hung.map(|other| other.volume_mm3).sum::<Scalar>();
                Some(Hung {
                    volume_mm3,
                    ..first
                })
            })
            .collect();
        self.tips = parents
            .iter()
            .map(|grown_from| {
                grown_from
                    .iter()
                    .fold(on_plate, |tips, parent| {
                        tips.saturating_add(self.tips[*parent as usize])
                    })
                    .min(STEADY)
            })
            .collect();
        self.grow_hung(cells);

        // A piece that is the only child of its only parent takes that parent's set over
        // rather than copying it, which is the whole of a wall going straight up.
        let mut children = vec![0u32; self.held.len()];
        for grown_from in &parents {
            for parent in grown_from {
                children[*parent as usize] += 1;
            }
        }

        let mut held = Vec::with_capacity(parts.count());
        for grown_from in &parents {
            let mut piece = match grown_from.as_slice() {
                [only] if children[*only as usize] == 1 => {
                    let mut taken = std::mem::take(&mut self.held[*only as usize]);
                    taken.prune(z, self.radius_mm);
                    taken
                }
                // Where pieces join, the largest set no other piece grows out of is taken
                // over and the rest poured into it, rather than every set copied afresh.
                parents => {
                    let largest = parents
                        .iter()
                        .filter(|parent| children[**parent as usize] == 1)
                        .max_by_key(|parent| self.held[**parent as usize].buckets.len())
                        .copied();
                    let mut piece = largest.map_or_else(Held::default, |largest| {
                        let mut taken = std::mem::take(&mut self.held[largest as usize]);
                        taken.prune(z, self.radius_mm);
                        taken
                    });
                    for parent in parents.iter().filter(|parent| Some(**parent) != largest) {
                        piece.take_in(&self.held[*parent as usize], z, self.radius_mm);
                    }
                    piece
                }
            };
            piece.buckets.shrink_to_fit();
            held.push(piece);
        }
        self.held = held;
        self.take_seeds(parts, z);
        self.take_lows(parts, z);
    }

    /// Gives every low point the climb has reached to the piece that begins at it, where
    /// nothing holds that piece yet.
    fn take_lows(&mut self, parts: &Parts, z: Scalar) {
        self.lows.reach(z + self.above_mm);
        let (tips, hung) = (&self.tips, &self.hung);
        let claimed = self.lows.claim(parts, |piece| {
            tips[piece as usize] == 0 && hung[piece as usize].is_none()
        });
        for (piece, point) in claimed {
            self.hung[piece as usize] = Some(Hung {
                point,
                volume_mm3: 0.0,
            });
        }
    }

    /// Adds the layer just climbed to onto what has grown out of every piece still hung.
    fn grow_hung(&mut self, cells: &[u64]) {
        if self.hung.iter().all(Option::is_none) {
            return;
        }
        for ((hung, tips), cells) in self.hung.iter_mut().zip(&self.tips).zip(cells) {
            if let Some(hung) = hung.as_mut().filter(|_| *tips == 0) {
                hung.volume_mm3 += *cells as Scalar * self.cell_mm3;
            }
        }
    }

    /// Where `piece` began, if it began at a low point or as a speck.
    fn hung(&self, piece: u32) -> Option<Hung> {
        self.hung.get(piece as usize).copied().flatten()
    }

    /// Hangs `piece`, a speck nothing holds, from `point`, with the resin of its first
    /// layer.
    fn speck(&mut self, piece: u32, point: Vec3, volume_mm3: Scalar) {
        let free = self.tips.get(piece as usize) == Some(&0);
        if let Some(hung) = self
            .hung
            .get_mut(piece as usize)
            .filter(|hung| free && hung.is_none())
        {
            *hung = Some(Hung { point, volume_mm3 });
        }
    }

    /// Puts a support under `piece` at `tip`, the point it hangs from.
    fn hang(&mut self, piece: u32, tip: Vec3) -> Vec3 {
        self.held[piece as usize].add(tip, self.reach_mm, self.radius_mm);
        self.count(piece);
        tip
    }

    /// Takes on every support handed to the run that the climb has reached.
    ///
    /// A seed the run itself placed sits a layer under the surface it holds, so the layer
    /// that first reaches it has nothing at its cell yet: it waits for the piece to
    /// appear, for as long as it could still hold one, rather than being lost and
    /// doubled up on.
    fn take_seeds(&mut self, parts: &Parts, z: Scalar) {
        while let Some(seed) = self.pending.pop_if(|seed| seed.0.z <= z + self.above_mm) {
            self.waiting.push(seed);
        }
        let floor_z = z - self.radius_mm;
        let mut waiting = std::mem::take(&mut self.waiting);
        waiting.retain(|(seed, cell)| {
            let Some(piece) = parts.at(cell[0], cell[1]) else {
                return seed.z >= floor_z;
            };
            self.held[piece as usize].add(*seed, self.reach_mm, self.radius_mm);
            // A part the hand has held is held as its owner wants it, see ADR 0095.
            self.tips[piece as usize] = STEADY;
            false
        });
        self.waiting = waiting;
    }

    /// The pieces of the layer climbed to that nothing holds, by their number.
    fn adrift(&self) -> impl Iterator<Item = u32> + '_ {
        (0u32..)
            .zip(&self.tips)
            .filter_map(|(piece, tips)| (*tips == 0).then_some(piece))
    }

    /// The pieces held, but at fewer places than a part can stand on.
    fn unsteady(&self) -> Vec<u32> {
        (0u32..)
            .zip(&self.tips)
            .filter_map(|(piece, tips)| (1..STEADY).contains(tips).then_some(piece))
            .collect()
    }

    fn count(&mut self, piece: u32) {
        let tips = &mut self.tips[piece as usize];
        *tips = (*tips + 1).min(STEADY);
    }

    /// Puts a support under `piece` at `at` to steady it, unless one within `clear_mm`
    /// already stands there; the ball a support holds is not asked about, because a part
    /// on one support is held and still not steady.
    fn prop(&mut self, piece: u32, at: Vec2, z: Scalar, clear_mm: Scalar) -> Option<Vec3> {
        let held = &mut self.held[piece as usize];
        if held.near(at, clear_mm.min(self.radius_mm), self.radius_mm) {
            return None;
        }
        let contact = at.extend(z);
        held.add(contact, self.reach_mm, self.radius_mm);
        Some(contact)
    }

    /// Stops asking after `piece`, which wanted a support where none may stand: a patch
    /// painted out is left alone, not held from further up. See `docs/decisions/0093`.
    fn leave(&mut self, piece: Option<u32>) {
        if let Some(tips) = piece.and_then(|piece| self.tips.get_mut(piece as usize)) {
            *tips = STEADY;
        }
    }

    /// Keeps the layer just sampled as what the next one grows out of.
    fn rest_on(&mut self, parts: Parts) {
        self.below = Some(parts);
    }

    /// Puts a support at `at` on the underside of `piece` at height `z`, holding a ball of
    /// `reach_mm`, unless one standing under the same piece already holds that place.
    /// Returns the contact it placed.
    fn take(&mut self, piece: Option<u32>, at: Vec2, z: Scalar, reach_mm: Scalar) -> Option<Vec3> {
        let contact = at.extend(z);
        let Some(piece) = piece.filter(|piece| (*piece as usize) < self.held.len()) else {
            return Some(contact);
        };
        let held = &mut self.held[piece as usize];
        if held.holds(at, z, self.radius_mm, self.above_mm) {
            return None;
        }
        held.add(contact, reach_mm, self.radius_mm);
        self.count(piece);
        Some(contact)
    }
}

/// The supports standing under one piece of a layer.
#[derive(Debug, Clone, Default)]
struct Held {
    buckets: FastMap<u64, Vec<Hold>>,
    /// The lowest height any support here still holds up to, so that a set with nothing
    /// to drop is not walked at all.
    spent_z: Scalar,
}

/// One support standing, and the radius of the ball it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hold {
    at: Vec3,
    reach_mm: Scalar,
}

impl Hold {
    /// The height past which this support holds nothing.
    fn spent_z(self) -> Scalar {
        self.at.z + self.reach_mm
    }
}

impl Held {
    /// Whether anything here already holds `at` on the layer whose contacts sit at `z`,
    /// each by its own ball, which is no wider than the side of a bucket.
    fn holds(&self, at: Vec2, z: Scalar, radius_mm: Scalar, above_mm: Scalar) -> bool {
        let home = bucket_of(at, radius_mm);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                self.buckets
                    .get(&packed([home[0] + dx, home[1] + dy]))
                    .is_some_and(|holds| {
                        holds.iter().any(|hold| {
                            let reach = reach_at(z - hold.at.z, hold.reach_mm, above_mm);
                            (hold.at.truncate() - at).length_squared() < reach * reach
                        })
                    })
            })
        })
    }

    /// Whether anything here stands within `clear_mm` of `at` on the plate, whatever its
    /// height; `clear_mm` is no more than the side of a bucket.
    fn near(&self, at: Vec2, clear_mm: Scalar, radius_mm: Scalar) -> bool {
        let home = bucket_of(at, radius_mm);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                self.buckets
                    .get(&packed([home[0] + dx, home[1] + dy]))
                    .is_some_and(|holds| {
                        holds.iter().any(|hold| {
                            (hold.at.truncate() - at).length_squared() < clear_mm * clear_mm
                        })
                    })
            })
        })
    }

    fn add(&mut self, contact: Vec3, reach_mm: Scalar, radius_mm: Scalar) {
        self.put(
            Hold {
                at: contact,
                reach_mm,
            },
            radius_mm,
        );
    }

    fn put(&mut self, hold: Hold, radius_mm: Scalar) {
        if self.buckets.is_empty() {
            self.spent_z = hold.spent_z();
        }
        self.spent_z = self.spent_z.min(hold.spent_z());
        self.buckets
            .entry(packed(bucket_of(hold.at.truncate(), radius_mm)))
            .or_default()
            .push(hold);
    }

    /// Drops everything that holds nothing at `z` or above.
    fn prune(&mut self, z: Scalar, radius_mm: Scalar) {
        if self.buckets.is_empty() || self.spent_z >= z {
            return;
        }
        let kept = std::mem::take(self);
        self.take_in(&kept, z, radius_mm);
    }

    /// Takes on everything of `other` that still holds something at `z` or above and is
    /// not here already: pieces that split and join again bring the same supports back,
    /// and a part split a hundred ways would otherwise hold each a hundred times.
    fn take_in(&mut self, other: &Self, z: Scalar, radius_mm: Scalar) {
        for hold in other.buckets.values().flatten() {
            if hold.spent_z() >= z && !self.has(*hold, radius_mm) {
                self.put(*hold, radius_mm);
            }
        }
    }

    fn has(&self, hold: Hold, radius_mm: Scalar) -> bool {
        self.buckets
            .get(&packed(bucket_of(hold.at.truncate(), radius_mm)))
            .is_some_and(|holds| holds.contains(&hold))
    }
}

/// How far a standing support still holds, `rise` millimetres above itself: a ball of the
/// coverage radius, so what is climbing away from it is on its own. See
/// `docs/decisions/0079`.
fn reach_at(rise: Scalar, radius_mm: Scalar, above_mm: Scalar) -> Scalar {
    // A seed sits on the surface and a placed contact a layer under it, so a support that
    // much above the place still counts as under it.
    let rise = (rise + above_mm).max(0.0);
    (radius_mm * radius_mm - rise * rise).max(0.0).sqrt()
}

/// A bucket as one word, which hashes in one step where a pair of integers hashes a byte
/// at a time.
fn packed([x, y]: [i32; 2]) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

fn bucket_of(at: Vec2, radius_mm: Scalar) -> [i32; 2] {
    [
        (at.x / radius_mm).floor() as i32,
        (at.y / radius_mm).floor() as i32,
    ]
}

/// One cell of `piece`: the middle of its bounds where the piece covers it, else its first.
fn middle_of(piece: &Field) -> Option<[i32; 2]> {
    let (min, max) = piece.bounds()?;
    let middle = [i32::midpoint(min[0], max[0]), i32::midpoint(min[1], max[1])];
    if piece.contains(middle[0], middle[1]) {
        return Some(middle);
    }
    let (y, row) = piece.rows().next()?;
    Some([row.first()?.x0, y])
}

/// The grid the whole stack is read on, or `None` when there is nothing on it.
fn grid_of(stack: &Sliced) -> Option<Grid> {
    let mut min = Vec2::splat(Scalar::INFINITY);
    let mut max = Vec2::splat(Scalar::NEG_INFINITY);
    for point in stack
        .layers
        .iter()
        .flat_map(|layer| &layer.contours)
        .flat_map(|contour| &contour.points)
    {
        min = min.min(*point);
        max = max.max(*point);
    }
    (min.x <= max.x).then(|| Grid::covering(min, max, GRID_PITCH_MM))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;
    use core_geometry::Mesh;
    use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};

    /// Layer height every test in this module cuts at, millimetres.
    const LAYER_HEIGHT_MM: Scalar = 0.05;

    fn profile() -> SupportProfile {
        crate::tests::profile()
    }

    fn slice(mesh: &Mesh) -> Sliced {
        PlaneSliceEngine
            .slice(
                mesh,
                &SliceSettings {
                    layer_height: LAYER_HEIGHT_MM,
                    ..SliceSettings::default()
                },
            )
            .expect("a closed mesh slices")
    }

    /// The contacts of [`super::generate_supports`], which is what most of these tests ask
    /// about.
    fn generate_supports(
        stack: &Sliced,
        layer_height_mm: Scalar,
        profile: &SupportProfile,
        seeds: &[Vec3],
        lows: &[Vec3],
        blocked: Option<&Blocked>,
        progress: &mut dyn FnMut(usize) -> bool,
    ) -> Vec<Vec3> {
        super::generate_supports(
            stack,
            layer_height_mm,
            profile,
            None,
            seeds,
            lows,
            blocked,
            progress,
        )
        .into_iter()
        .map(|need| need.contact)
        .collect()
    }

    /// Everything automatic placement puts under `mesh`, with nothing there already.
    fn supports_for(mesh: &Mesh) -> Vec<Vec3> {
        generate_supports(
            &slice(mesh),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        )
    }

    fn merged(a: &Mesh, b: &Mesh) -> Mesh {
        let offset = a.vertices.len() as u32;
        let mut mesh = a.clone();
        mesh.vertices.extend_from_slice(&b.vertices);
        mesh.faces.extend(
            b.faces
                .iter()
                .map(|[x, y, z]| [x + offset, y + offset, z + offset]),
        );
        mesh
    }

    /// A ball of `radius` with its south pole at `z`, fine enough that its lower cap is a
    /// smooth overhang rather than a staircase.
    fn sphere(radius: Scalar, base_mm: Scalar) -> Mesh {
        let (segments, rings) = (96u32, 48u32);
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for ring in 0..=rings {
            let phi = std::f32::consts::PI * ring as Scalar / rings as Scalar;
            for segment in 0..segments {
                let theta = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
                vertices.push(Vec3::new(
                    radius * phi.sin() * theta.cos(),
                    radius * phi.sin() * theta.sin(),
                    base_mm + radius - radius * phi.cos(),
                ));
            }
        }
        for ring in 0..rings {
            for segment in 0..segments {
                let next = (segment + 1) % segments;
                let low_left = ring * segments + segment;
                let low_right = ring * segments + next;
                let high_left = (ring + 1) * segments + segment;
                let high_right = (ring + 1) * segments + next;
                faces.push([low_left, high_right, high_left]);
                faces.push([low_left, low_right, high_right]);
            }
        }
        Mesh::new(vertices, faces)
    }

    /// A cone standing on its point `apex_mm` over the plate, opening to `radius` over
    /// `height`: nothing on it leans further than its own wall.
    fn cone_on_its_point(apex_mm: Scalar, radius: Scalar, height: Scalar) -> Mesh {
        let sides = 64u32;
        let mut vertices = vec![
            Vec3::new(0.0, 0.0, apex_mm),
            Vec3::new(0.0, 0.0, apex_mm + height),
        ];
        let mut faces = Vec::new();
        for side in 0..sides {
            let angle = std::f32::consts::TAU * side as Scalar / sides as Scalar;
            vertices.push(Vec3::new(
                radius * angle.cos(),
                radius * angle.sin(),
                apex_mm + height,
            ));
            let (here, next) = (2 + side, 2 + (side + 1) % sides);
            faces.push([0, next, here]);
            faces.push([1, here, next]);
        }
        Mesh::new(vertices, faces)
    }

    /// What placement wants under `mesh`, with what each contact holds.
    fn needs_for(mesh: &Mesh) -> Vec<Need> {
        super::generate_supports(
            &slice(mesh),
            LAYER_HEIGHT_MM,
            &profile(),
            None,
            &[],
            &[],
            None,
            &mut |_| true,
        )
    }

    /// A 10 × 10 × 1 mm shelf on a 2 mm square post 5 mm tall, with a 10 × 10 mm tower
    /// `tower_mm` tall on top of it.
    fn shelf(tower_mm: Scalar) -> Mesh {
        let post = crate::tests::box_mesh(Vec3::new(9.0, 9.0, 0.0), Vec3::new(11.0, 11.0, 5.0));
        let slab = crate::tests::box_mesh(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 6.0));
        let mesh = merged(&post, &slab);
        if tower_mm <= 0.0 {
            return mesh;
        }
        let tower = Vec3::new(15.0, 15.0, 6.0 + tower_mm);
        merged(
            &mesh,
            &crate::tests::box_mesh(Vec3::new(5.0, 5.0, 6.0), tower),
        )
    }

    /// The model above every overhang contact under `mesh`, mm³.
    fn carried(mesh: &Mesh) -> Vec<Scalar> {
        needs_for(mesh)
            .into_iter()
            .filter_map(|need| match need.reason {
                Reason::Overhang { carried_mm3 } => Some(carried_mm3),
                Reason::Island { .. } | Reason::Step { .. } | Reason::Steady => None,
            })
            .collect()
    }

    #[test]
    fn a_floating_slab_is_held_as_an_island_of_its_own_area() {
        let slab = crate::tests::box_mesh(Vec3::new(2.0, 2.0, 5.0), Vec3::new(6.0, 5.0, 6.0));
        let needs = needs_for(&slab);
        let islands: Vec<Scalar> = needs
            .iter()
            .filter_map(|need| match need.reason {
                Reason::Island { area_mm2 } => Some(area_mm2),
                Reason::Overhang { .. } | Reason::Step { .. } | Reason::Steady => None,
            })
            .collect();
        assert!(!islands.is_empty());
        assert!(
            islands.iter().all(|mm2| (mm2 - 12.0).abs() < 0.5),
            "the slab is 4 × 3 mm, so its island is 12 mm², got {islands:?}"
        );
        assert!(
            !needs
                .iter()
                .any(|need| matches!(need.reason, Reason::Overhang { .. })),
            "nothing on a slab with nothing beneath it is an overhang"
        );
    }

    #[test]
    fn an_overhang_carries_the_model_above_it() {
        let alone = carried(&shelf(0.0));
        let under_tower = carried(&shelf(20.0));
        assert!(!alone.is_empty() && !under_tower.is_empty());
        // The slab is 100 mm³ less its post and the layer the contacts are cut under; the
        // tower adds 2000 mm³ more.
        assert!(
            alone.iter().all(|mm3| (90.0..=100.0).contains(mm3)),
            "a bare shelf carries itself, got {alone:?}"
        );
        assert!(
            under_tower
                .iter()
                .all(|mm3| (2090.0..=2100.0).contains(mm3)),
            "a shelf under a 20 mm tower carries the tower too, got {under_tower:?}"
        );
    }

    #[test]
    fn a_point_hanging_over_the_plate_is_held_once_it_is_wide_enough_to_print() {
        // A wall 27 degrees from vertical, well inside the 45 the profile allows, so
        // nothing on the cone is an overhang and its first layer is a speck.
        let apex_mm = 5.0;
        let points = supports_for(&cone_on_its_point(apex_mm, 5.0, 10.0));

        let lowest = points
            .iter()
            .map(|point| point.z)
            .fold(Scalar::INFINITY, Scalar::min);
        // The cone is `MIN_ISLAND_MM` across where its radius is half of that, and its
        // radius is half its height above the point: 0.4 mm up.
        assert!(
            lowest < apex_mm + 1.0,
            "a part with nothing under it hangs in mid-air; its lowest contact is at \
             {lowest} mm and its point at {apex_mm} mm"
        );
    }

    #[test]
    fn a_small_ball_stands_on_more_than_the_support_under_its_pole() {
        let points = supports_for(&sphere(5.0, 5.0));
        assert!(
            points.len() >= 4,
            "a part on fewer than three supports rocks on the peel: one under the pole and \
             three round it, got {}",
            points.len()
        );
    }

    #[test]
    fn every_corner_of_a_floating_slab_is_held() {
        // Off the lattice on every side, so no corner has a lattice point to lean on.
        let (low, high) = (Vec2::new(1.3, 2.7), Vec2::new(18.3, 15.7));
        let points = supports_for(&box_mesh(low.extend(10.0), high.extend(14.0)));
        let inset_mm = SampleConfig::for_profile(&profile()).edge_inset_mm;

        for corner in [
            low,
            Vec2::new(high.x, low.y),
            high,
            Vec2::new(low.x, high.y),
        ] {
            let nearest = points
                .iter()
                .map(|point| (point.truncate() - corner).length())
                .fold(Scalar::INFINITY, Scalar::min);
            assert!(
                nearest <= inset_mm * Scalar::sqrt(2.0) + 2.0 * GRID_PITCH_MM,
                "the corner at {corner} is {nearest} mm from the nearest support, and a \
                 corner is held one inset in from both of its sides"
            );
        }
    }

    #[test]
    fn no_head_under_a_floating_slab_reaches_over_its_edge() {
        let (low, high) = (Vec2::new(1.3, 2.7), Vec2::new(18.3, 15.7));
        let points = supports_for(&box_mesh(low.extend(10.0), high.extend(14.0)));
        let radius_mm = profile().contact_radius_mm();

        assert!(!points.is_empty(), "a floating slab is an island");
        for point in &points {
            let at = point.truncate();
            let room_mm = (at - low).min_element().min((high - at).min_element());
            assert!(
                room_mm >= radius_mm,
                "the head at {at} is {room_mm} mm from the slab's edge and {radius_mm} mm \
                 wide, so it climbs the wall above that edge"
            );
        }
    }

    #[test]
    fn a_part_that_begins_at_a_point_is_held_at_the_point_itself() {
        let apex = Vec3::new(0.0, 0.0, 5.0);
        let cone = cone_on_its_point(apex.z, 5.0, 10.0);
        let lows = crate::low_points(&cone, core_geometry::Transform::default());
        let points = generate_supports(
            &slice(&cone),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &lows,
            None,
            &mut |_| true,
        );

        assert!(
            points.contains(&apex),
            "the cone hangs from its point at {apex}, and its lowest contact is {:?}",
            points.iter().min_by(|a, b| a.z.total_cmp(&b.z))
        );
    }

    #[test]
    fn a_low_point_on_a_part_that_is_already_held_gets_no_support_of_its_own() {
        // A box on the plate, told of a point on its wall: nothing begins there.
        let standing = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
        let points = generate_supports(
            &slice(&standing),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[Vec3::new(0.0, 5.0, 5.0)],
            None,
            &mut |_| true,
        );
        assert!(points.is_empty(), "got {points:?}");
    }

    /// Two boxes, the upper one jutting `step` further out over the lower one's ledge:
    /// a staircase of overhangs, one above the other, which is what a stepped model is.
    fn steps(step: Scalar, rise: Scalar) -> Mesh {
        let lower = box_mesh(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(10.0, 10.0, 10.0 + rise),
        );
        let upper = box_mesh(
            Vec3::new(0.0, -step, 10.0 + rise),
            Vec3::new(10.0, 10.0, 10.0 + rise + rise),
        );
        merged(&lower, &upper)
    }

    /// What a run sized by the shipped automatic profile wants under `mesh`.
    fn sized_needs_for(mesh: &Mesh) -> Vec<Need> {
        let catalogue = printer_profiles::Catalogue::bundled().expect("the shipped catalogue");
        let auto = &catalogue
            .autos()
            .next()
            .expect("an automatic profile ships")
            .profile;
        super::generate_supports(
            &slice(mesh),
            LAYER_HEIGHT_MM,
            // Placed by the automatic profile's numbers, as the window places a sized run.
            &auto.placing_with(&profile()),
            Some(auto),
            &[],
            &[],
            None,
            &mut |_| true,
        )
    }

    /// A block 20 mm square standing on the plate, a 20 by 12 mm shelf put out in one
    /// layer at z = 5 when `shelf`, and on that a block 4 mm tall whose face over the shelf
    /// leans 30 degrees out: past a step's 20, short of the fixture's 45.
    fn shelf_under_a_lean(shelf: bool) -> Mesh {
        let base_y = if shelf { 0.0 } else { -12.0 };
        let base = box_mesh(Vec3::new(0.0, base_y, 0.0), Vec3::new(20.0, 20.0, 5.0));
        let slab = box_mesh(Vec3::new(0.0, -12.0, 5.0), Vec3::new(20.0, 20.0, 6.0));
        let mut lean = box_mesh(Vec3::new(0.0, -12.0, 6.0), Vec3::new(20.0, 20.0, 10.0));
        let out = 4.0 * 30.0_f32.to_radians().tan();
        // The two corners of the top face over the shelf's edge.
        for corner in [4, 5] {
            lean.vertices[corner].y -= out;
        }
        merged(&merged(&base, &slab), &lean)
    }

    /// The contacts under the leaning face, above the shelf's edge, with why each is there.
    fn under_the_lean(needs: &[Need]) -> Vec<Reason> {
        needs
            .iter()
            .filter(|need| need.contact.y < -12.3 && need.contact.z > 6.2)
            .map(|need| need.reason)
            .collect()
    }

    #[test]
    fn a_shelf_is_held_as_a_step_of_the_resin_it_puts_out() {
        let needs = sized_needs_for(&shelf_under_a_lean(true));
        let steps: Vec<Scalar> = needs
            .iter()
            .filter(|need| (need.contact.z - 5.0).abs() < 0.2)
            .filter_map(|need| match need.reason {
                Reason::Step { new_mm3 } => Some(new_mm3),
                _ => None,
            })
            .collect();
        assert!(!steps.is_empty(), "the shelf is held as a step");
        assert!(
            steps.iter().all(|mm3| (11.0..=12.5).contains(mm3)),
            "240 mm² of shelf in a 0.05 mm layer is 12 mm³, got {steps:?}"
        );
    }

    #[test]
    fn a_face_leaning_past_a_steps_angle_is_held_where_the_step_reaches_it() {
        let held = under_the_lean(&sized_needs_for(&shelf_under_a_lean(true)));
        assert!(
            !held.is_empty(),
            "the 30 degree face over the shelf is held"
        );
        assert!(
            held.iter()
                .all(|reason| matches!(reason, Reason::Step { .. })),
            "as part of the step, got {held:?}"
        );
        assert!(
            under_the_lean(&sized_needs_for(&shelf_under_a_lean(false)))
                .iter()
                .all(|reason| matches!(reason, Reason::Steady)),
            "the same face with no step under it is only steadied"
        );
        assert!(
            under_the_lean(&needs_for(&shelf_under_a_lean(true))).is_empty(),
            "and a run that does not size its supports holds a step as it always has"
        );
    }

    /// A block 20 mm wide on the plate whose face along -y leans 15 degrees out over 30 mm
    /// of rise: too gentle to sag, steep enough to steady a part on.
    fn leaning_wall() -> Mesh {
        let mut wall = box_mesh(Vec3::ZERO, Vec3::new(20.0, 10.0, 30.0));
        let out = 30.0 * 15.0_f32.to_radians().tan();
        for corner in [4, 5] {
            wall.vertices[corner].y -= out;
        }
        wall
    }

    #[test]
    fn a_gentle_slope_is_steadied_sparsely_and_only_when_sized() {
        let on_the_face = |needs: Vec<Need>| -> Vec<Need> {
            needs
                .into_iter()
                .filter(|need| need.contact.y < -0.5)
                .collect()
        };
        let steadied = on_the_face(sized_needs_for(&leaning_wall()));
        assert!(!steadied.is_empty(), "a 15 degree face is steadied on");
        assert!(
            steadied
                .iter()
                .all(|need| matches!(need.reason, Reason::Steady)),
            "and only steadied"
        );
        for (index, one) in steadied.iter().enumerate() {
            for other in &steadied[index + 1..] {
                assert!(
                    one.contact.distance(other.contact) > 5.0,
                    "steadying supports stand far apart: {} and {}",
                    one.contact,
                    other.contact
                );
            }
        }
        assert!(
            on_the_face(needs_for(&leaning_wall())).is_empty(),
            "a run that does not size its supports leaves it alone"
        );
    }

    #[test]
    fn a_step_above_another_one_is_held_as_well() {
        // One millimetre of jut in a single layer, four millimetres above the ledge under
        // it: too wide for the resin to bridge, and too far up for the supports below to
        // reach. See `docs/decisions/0079` and `docs/decisions/0080`.
        let points = supports_for(&steps(1.0, 4.0));
        let upper = points.iter().filter(|point| point.y < 0.0).count();

        // Ten millimetres of shelf at the medium preset's five millimetre spacing.
        assert!(
            upper >= 2,
            "the upper step is 10 mm of shelf and got {upper} supports"
        );
    }

    /// The region every face of `mesh` looking down belongs to: the underside a support
    /// would otherwise be put against.
    fn underside(mesh: &Mesh) -> core_geometry::Region {
        let mut region = core_geometry::Region::default();
        for face in 0..mesh.faces.len() {
            let looking_down = mesh.triangle(face).is_some_and(|triangle| {
                triangle.normal_unnormalized().normalize_or_zero().z < -0.9
            });
            region.set(face, looking_down);
        }
        region
    }

    #[test]
    fn an_island_painted_out_of_bounds_is_left_alone() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let blocked = Blocked::new(
            &floating,
            &underside(&floating),
            core_geometry::Transform::default(),
        )
        .expect("the underside was painted");

        let points = generate_supports(
            &slice(&floating),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            Some(&blocked),
            &mut |_| true,
        );

        assert!(
            points.is_empty(),
            "the only surface that needed holding up was blocked, got {} contacts",
            points.len()
        );
    }

    #[test]
    fn a_box_standing_on_the_plate_needs_no_supports() {
        let standing = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
        assert!(supports_for(&standing).is_empty());
    }

    #[test]
    fn a_box_floating_over_the_plate_is_an_island_and_is_held_from_underneath() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let points = supports_for(&floating);

        assert!(!points.is_empty(), "an island must be held up");
        for point in &points {
            assert!(
                (0.0..=10.0).contains(&point.x) && (0.0..=10.0).contains(&point.y),
                "{point} is not under the island"
            );
            assert!(
                (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
                "{point} is not on the underside of the island"
            );
        }
    }

    #[test]
    fn an_island_is_held_at_the_spacing_the_profile_asks_for() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
        let points = supports_for(&floating);
        let spacing = SampleConfig::for_profile(&profile()).spacing_mm;

        for row in 1..30 {
            for column in 1..30 {
                let probe = Vec2::new(column as Scalar, row as Scalar);
                let nearest = points
                    .iter()
                    .map(|point| (point.truncate() - probe).length())
                    .fold(Scalar::INFINITY, Scalar::min);
                assert!(
                    nearest <= spacing,
                    "{probe} is {nearest} mm from the nearest support, past {spacing}"
                );
            }
        }
    }

    #[test]
    fn a_ball_resting_on_the_plate_is_held_under_its_lower_cap() {
        let points = supports_for(&sphere(15.0, 0.0));
        assert!(
            points.len() >= 8,
            "a 30 mm ball hangs over nothing all round its lower cap, got {} supports",
            points.len()
        );

        // Everything leaning further than 45 degrees is below the ball's own equator by
        // the radius over root two, and its supports stand under that cap.
        for point in &points {
            assert!(
                point.z <= 15.0 - 15.0 / 2.0_f32.sqrt() + 1.0,
                "{point} is above the latitude that holds itself up"
            );
        }
    }

    #[test]
    fn a_wall_leaning_inside_the_angle_holds_itself_up() {
        // A pyramid standing on its base: every face leans 45 degrees inwards.
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(20.0, 20.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(10.0, 10.0, 10.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 1, 4],
                [1, 2, 4],
                [2, 3, 4],
                [3, 0, 4],
            ],
        );
        assert!(supports_for(&mesh).is_empty());
    }

    #[test]
    fn a_table_on_a_leg_is_held_around_its_coast_and_not_over_the_leg() {
        let leg = box_mesh(Vec3::new(10.0, 10.0, 0.0), Vec3::new(14.0, 14.0, 10.0));
        let table = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(24.0, 24.0, 12.0));
        let points = supports_for(&merged(&leg, &table));

        assert!(points.len() >= 4, "a 24 mm table needs holding all round");
        for point in &points {
            assert!(
                (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
                "{point} is not on the underside of the table"
            );
            let over_the_leg = (10.0..=14.0).contains(&point.x) && (10.0..=14.0).contains(&point.y);
            assert!(!over_the_leg, "{point} stands where the leg already holds");
        }
    }

    #[test]
    fn a_support_that_is_already_there_holds_what_is_around_it() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(4.0, 4.0, 14.0));
        assert!(!supports_for(&floating).is_empty(), "a bare island is held");

        let seeded = generate_supports(
            &slice(&floating),
            LAYER_HEIGHT_MM,
            &profile(),
            &[Vec3::new(2.0, 2.0, 10.0)],
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            seeded.is_empty(),
            "a hand-placed support must not be doubled up on, got {seeded:?}"
        );
    }

    #[test]
    fn a_second_run_over_what_the_first_one_placed_adds_nothing() {
        // A slab and a ball: flat undersides, where a contact sits right under the first
        // layer, and a curved one, where the layers it holds go on widening above it.
        for mesh in [
            box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(20.0, 20.0, 14.0)),
            sphere(8.0, 6.0),
        ] {
            let stack = slice(&mesh);
            let first = supports_for(&mesh);
            assert!(!first.is_empty(), "the part needs holding");

            let again = generate_supports(
                &stack,
                LAYER_HEIGHT_MM,
                &profile(),
                &first,
                &[],
                None,
                &mut |_| true,
            );
            assert!(
                again.is_empty(),
                "every place is held by the {} supports already there, and the run put \
                 {} more beside them",
                first.len(),
                again.len()
            );
        }
    }

    #[test]
    fn a_support_under_one_island_does_not_hold_the_island_over_it() {
        // Two slabs with 0.5 mm of air between them. The upper one is well inside the
        // reach of what holds the lower one, and it is a part of its own; see
        // `docs/decisions/0081`.
        let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 11.0));
        let upper = box_mesh(Vec3::new(0.0, 0.0, 11.5), Vec3::new(10.0, 10.0, 12.5));
        let points = supports_for(&merged(&lower, &upper));

        let held_at = |z: Scalar| {
            points
                .iter()
                .filter(|point| (point.z - z).abs() < 0.5)
                .count()
        };
        assert!(held_at(10.0) >= 4, "the lower island is held");
        assert!(
            held_at(11.5) >= 4,
            "the upper island is a part of its own and is held on its own account, got \
             {} supports under it",
            held_at(11.5)
        );
    }

    /// A wedge 30 mm across and 30 mm deep hanging from its top face, its underside
    /// leaning `lean_deg` from vertical and its lowest edge at z = 10, wound outwards.
    fn hanging_wedge(lean_deg: Scalar) -> Mesh {
        let (run, depth) = (30.0, 15.0);
        let top = 10.0 + run / lean_deg.to_radians().tan();
        let vertices = vec![
            Vec3::new(0.0, -depth, top),
            Vec3::new(run, -depth, top),
            Vec3::new(run, -depth, 10.0),
            Vec3::new(0.0, depth, top),
            Vec3::new(run, depth, top),
            Vec3::new(run, depth, 10.0),
        ];
        let faces = vec![
            [0, 2, 1],
            [3, 4, 5],
            [0, 1, 4],
            [0, 4, 3],
            [1, 2, 5],
            [1, 5, 4],
            [0, 5, 2],
            [0, 3, 5],
        ];
        Mesh::new(vertices, faces)
    }

    /// Supports per square millimetre of plate under the middle of a hanging wedge.
    fn held_per_mm2(lean_deg: Scalar, profile: &SupportProfile) -> Scalar {
        let points = generate_supports(
            &slice(&hanging_wedge(lean_deg)),
            LAYER_HEIGHT_MM,
            profile,
            &[],
            &[],
            None,
            &mut |_| true,
        );
        let inside = points
            .iter()
            .filter(|point| (4.0..26.0).contains(&point.x) && (-11.0..11.0).contains(&point.y))
            .count();
        inside as Scalar / (22.0 * 22.0)
    }

    #[test]
    fn a_face_just_past_the_angle_is_held_more_sparsely_than_a_ceiling() {
        let mut profile = profile();
        profile.steep_contact_spacing_mm = 2.0 * profile.contact_spacing_mm;
        let (steep, flat) = (held_per_mm2(50.0, &profile), held_per_mm2(85.0, &profile));
        assert!(
            flat > 0.0 && steep < 0.6 * flat,
            "twice the spacing at the angle holds a steep face far more sparsely; got \
             {steep} supports per mm2 under the steep face and {flat} under the flat one"
        );
    }

    #[test]
    fn a_needle_hanging_under_a_part_is_held_at_its_tip() {
        // Never as wide as an island anywhere along it, but far more than dust.
        let needle = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.3, 0.3, 13.0));
        let part = box_mesh(Vec3::new(-5.0, -5.0, 13.0), Vec3::new(5.0, 5.0, 15.0));
        let points = supports_for(&merged(&needle, &part));

        assert!(
            points.iter().any(|point| {
                (0.0..=0.3).contains(&point.x)
                    && (0.0..=0.3).contains(&point.y)
                    && (point.z - 10.0).abs() <= 2.0 * LAYER_HEIGHT_MM
            }),
            "the needle's tip at z = 10 hangs in mid-air, got {points:?}"
        );
    }

    #[test]
    fn supports_brought_back_by_pieces_that_split_and_join_are_held_once() {
        let mut split = Held::default();
        split.add(Vec3::new(1.0, 2.0, 3.0), 1.0, 1.0);
        let mut joined = Held::default();
        for _ in 0..100 {
            joined.take_in(&split, 0.0, 1.0);
        }
        assert_eq!(joined.buckets.values().flatten().count(), 1);
    }

    #[test]
    fn a_speck_too_small_to_print_is_left_alone() {
        let speck = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.15, 0.15, 10.4));
        assert!(supports_for(&speck).is_empty());
    }

    #[test]
    fn a_shallower_angle_puts_down_more_supports() {
        let ball = sphere(15.0, 0.0);
        let stack = slice(&ball);
        let loose = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                max_overhang_deg: 60.0,
                ..profile()
            },
            &[],
            &[],
            None,
            &mut |_| true,
        );
        let strict = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                max_overhang_deg: 30.0,
                ..profile()
            },
            &[],
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            strict.len() > loose.len(),
            "30 degrees placed {} against 60 degrees' {}",
            strict.len(),
            loose.len()
        );
    }

    #[test]
    fn a_denser_profile_puts_down_more_supports() {
        let ball = sphere(15.0, 0.0);
        let stack = slice(&ball);
        let normal = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        );
        let dense = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                density: 3.0,
                ..profile()
            },
            &[],
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            dense.len() > normal.len(),
            "three times the density put down {} against {}",
            dense.len(),
            normal.len()
        );
    }

    #[test]
    fn a_run_that_is_stopped_before_it_starts_places_nothing() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
        let stack = slice(&floating);

        let mut looked = 0;
        let stopped = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| {
                looked += 1;
                false
            },
        );
        assert!(stopped.is_empty());
        assert_eq!(
            looked, 1,
            "the run stopped at the first layer it asked about"
        );
    }

    #[test]
    fn stopping_part_way_keeps_what_the_run_had_already_found() {
        let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
        let upper = box_mesh(Vec3::new(20.0, 0.0, 20.0), Vec3::new(30.0, 10.0, 22.0));
        let stack = slice(&merged(&lower, &upper));

        let half = stack.layers.len() / 2;
        let stopped = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |layer| layer < half,
        );

        assert!(!stopped.is_empty(), "the lower island was reached");
        for point in &stopped {
            assert!(point.x < 20.0, "{point} is on the island never reached");
        }
        let whole = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        );
        assert!(whole.len() > stopped.len());
    }

    #[test]
    fn the_same_model_always_produces_the_same_supports() {
        let ball = sphere(15.0, 3.0);
        let stack = slice(&ball);
        let once = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        );
        let twice = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        );
        assert_eq!(once, twice);
    }

    #[test]
    fn an_empty_stack_produces_nothing() {
        let points = generate_supports(
            &Sliced::default(),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            &[],
            None,
            &mut |_| true,
        );
        assert!(points.is_empty());
    }
}
