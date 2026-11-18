use core_geometry::{BoundingVolume, Bvh, Mesh, Scalar, Vec3};
use core_supports::Trapped;
use core_volume::{DrainHole, HoleSize, Toward, drill, holes_out};

use crate::bake::Baked;
use crate::error::EngineError;
use crate::trapped::trapped_in;

/// The most holes tried for one pocket in each role. A search scans the model once to find
/// the pockets, once a round for drains and for vents, and once more for what is left (ADR 0401).
pub const MOST_TRIES: usize = 4;

/// How far apart two mouths stand, in hole diameters, so the wall between them holds.
const SPACING_DIAMETERS: Scalar = 2.0;

/// What a drain search looks through, in the pose the model prints in: plate millimetres, +Z up.
#[derive(Debug, Clone, Copy)]
pub struct DrainSearch<'a> {
    /// What prints: the shell, its cavity and whatever stands in it.
    pub mesh: &'a Mesh,
    /// Bodies already cut out of it, wound inward (ADR 0075).
    pub cuts: Option<&'a Mesh>,
    pub size: HoleSize,
    /// The wall the cavity was cut to and its lattice, millimetres, which a drain is carried
    /// through (`pierce`); `None` for a model not hollowed here.
    pub wall: Option<(Scalar, Scalar)>,
    /// Whether each drained pocket also gets a vent, higher up, to let air in.
    pub vents: bool,
    /// The plate's height under a model that prints standing on it; `None` for one lifted.
    pub plate_z: Option<Scalar>,
    /// The layer the scan proving each hole cuts, millimetres.
    pub layer_height_mm: Scalar,
}

/// What a hole a search chose is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleRole {
    /// Low on the pocket, where the resin runs out.
    Drain,
    /// High on it, where the air comes in.
    Vent,
}

/// One hole a search chose, proven to reach its pocket, which is an index into
/// [`Drained::pockets`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuggestedHole {
    pub hole: DrainHole,
    pub pocket: usize,
    pub role: HoleRole,
}

/// What a drain search found and chose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drained {
    /// The pockets trapped before any hole of its own.
    pub pockets: Vec<Trapped>,
    pub holes: Vec<SuggestedHole>,
    /// What is still trapped with every hole drilled.
    pub trapped: Vec<Trapped>,
    /// How many times the model was sliced and scanned.
    pub scans: usize,
}

impl Drained {
    /// Every hole chosen, drains and vents.
    pub fn drain_holes(&self) -> impl Iterator<Item = DrainHole> + '_ {
        self.holes.iter().map(|suggested| suggested.hole)
    }

    /// Whether pocket `index` has no resin left trapped in it.
    pub fn is_drained(&self, index: usize) -> bool {
        self.pockets
            .get(index)
            .is_some_and(|pocket| !still_trapped(pocket, &self.trapped))
    }
}

/// Drain holes for every pocket of resin `search.mesh` traps, each proven by drilling it and
/// scanning again, and a vent above each drain that is proven the same way; see
/// `docs/design/hollowing.md`, "Suggested drains". A pocket no candidate drains stays trapped.
pub fn suggest_drains(search: &DrainSearch) -> Result<Drained, EngineError> {
    let found = suggest_drains_heard(search, |_| true)?;
    Ok(found.unwrap_or_default())
}

/// [`suggest_drains`], telling `heard` where each scan has got before it is cut and before
/// each window is read; answering false stops the search, and the answer is `None`.
pub fn suggest_drains_heard(
    search: &DrainSearch,
    mut heard: impl FnMut(Scanning) -> bool,
) -> Result<Option<Drained>, EngineError> {
    match search_heard(search, &mut heard) {
        Ok(drained) => Ok(Some(drained)),
        Err(Halt::Asked) => Ok(None),
        Err(Halt::Failed(error)) => Err(error),
    }
}

/// Where a drain search has got: the scan in hand, counted from 0, and its layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scanning {
    pub scan: usize,
    /// The most scans the search can take: it may end sooner.
    pub most_scans: usize,
    pub layers_read: usize,
    pub layers: usize,
}

impl Scanning {
    /// The share of the most the search can take that is done.
    pub fn share(&self) -> f32 {
        let within = match self.layers {
            0 => 0.0,
            layers => self.layers_read.min(layers) as f32 / layers as f32,
        };
        ((self.scan as f32 + within) / self.most_scans.max(1) as f32).min(1.0)
    }
}

/// Why a search ended before its answer.
enum Halt {
    Asked,
    Failed(EngineError),
}

impl From<EngineError> for Halt {
    fn from(error: EngineError) -> Self {
        Self::Failed(error)
    }
}

fn search_heard(
    search: &DrainSearch,
    heard: &mut dyn FnMut(Scanning) -> bool,
) -> Result<Drained, Halt> {
    let mut base = search.mesh.clone();
    if let Some(cuts) = search.cuts {
        base.append(cuts);
    }
    let rounds = if search.vents { 2 } else { 1 };
    let mut scans = Scans {
        search,
        base,
        count: 0,
        most: 2 + rounds * MOST_TRIES,
        heard,
    };
    let pockets = scans.trapped(&[])?;
    if pockets.is_empty() {
        return Ok(Drained {
            scans: scans.count,
            ..Drained::default()
        });
    }

    let bvh = Bvh::build(search.mesh);
    let mut taken = Vec::new();
    let every: Vec<usize> = (0..pockets.len()).collect();
    let drains = scans.prove(&pockets, &every, Toward::Down, &bvh, &mut taken)?;
    let mut holes = suggested(&drains, HoleRole::Drain);
    if search.vents {
        let drained: Vec<usize> = drains.iter().map(|(pocket, _)| *pocket).collect();
        let vents = scans.prove(&pockets, &drained, Toward::Up, &bvh, &mut taken)?;
        holes.extend(suggested(&vents, HoleRole::Vent));
    }
    let trapped = if holes.is_empty() {
        pockets.clone()
    } else {
        let all: Vec<DrainHole> = holes.iter().map(|suggested| suggested.hole).collect();
        scans.trapped(&all)?
    };
    Ok(Drained {
        pockets,
        holes,
        trapped,
        scans: scans.count,
    })
}

fn suggested(chosen: &[(usize, DrainHole)], role: HoleRole) -> Vec<SuggestedHole> {
    chosen
        .iter()
        .map(|&(pocket, hole)| SuggestedHole { hole, pocket, role })
        .collect()
}

/// The model a search scans, how often it has, and who hears it.
struct Scans<'a> {
    search: &'a DrainSearch<'a>,
    /// The model with the cuts it already had.
    base: Mesh,
    count: usize,
    most: usize,
    heard: &'a mut dyn FnMut(Scanning) -> bool,
}

impl Scans<'_> {
    /// What is trapped with `holes` drilled besides the cuts the model had.
    fn trapped(&mut self, holes: &[DrainHole]) -> Result<Vec<Trapped>, Halt> {
        let Some(material) = self.search.mesh.aabb() else {
            return Ok(Vec::new());
        };
        let mut solid = self.base.clone();
        if !holes.is_empty() {
            solid.append(&drill(holes, &[]).map_err(EngineError::Drill)?);
        }
        let baked = Baked {
            solid: solid.into(),
            ceiling_mm: material.maxs.z,
            floor_mm: material.mins.z,
        };
        let (scan, most_scans) = (self.count, self.most);
        self.count += 1;
        let heard = &mut *self.heard;
        let found = trapped_in(
            &baked,
            self.search.layer_height_mm,
            |layers_read, layers| {
                heard(Scanning {
                    scan,
                    most_scans,
                    layers_read,
                    layers,
                })
            },
        )?;
        found.ok_or(Halt::Asked)
    }

    /// The holes into `pocket` looked for `toward`, best first, carried through the wall.
    fn proposals(&self, pocket: &Trapped, toward: Toward, bvh: &Bvh) -> Vec<DrainHole> {
        let search = self.search;
        holes_out(
            search.mesh,
            bvh,
            pocket.at,
            toward,
            search.size,
            search.plate_z,
            search.wall,
        )
    }

    /// A hole for each of `wanted` of `pockets`, looked for `toward`, that drilled alone with
    /// the holes chosen so far empties its pocket: up to [`MOST_TRIES`] rounds of one try a
    /// pocket and one scan a round. Every mouth chosen joins `taken`.
    fn prove(
        &mut self,
        pockets: &[Trapped],
        wanted: &[usize],
        toward: Toward,
        bvh: &Bvh,
        taken: &mut Vec<Vec3>,
    ) -> Result<Vec<(usize, DrainHole)>, Halt> {
        let mut candidates: Vec<(usize, std::vec::IntoIter<DrainHole>)> = wanted
            .iter()
            .map(|&pocket| {
                (
                    pocket,
                    self.proposals(&pockets[pocket], toward, bvh).into_iter(),
                )
            })
            .collect();
        let spacing = SPACING_DIAMETERS * self.search.size.diameter_mm;
        let mut chosen: Vec<(usize, DrainHole)> = Vec::new();
        for _ in 0..MOST_TRIES {
            let mut round: Vec<Vec3> = taken.iter().copied().chain(mouths(&chosen)).collect();
            let tries: Vec<(usize, DrainHole)> = candidates
                .iter_mut()
                .filter(|(pocket, _)| !chosen.iter().any(|(served, _)| served == pocket))
                .filter_map(|(pocket, left)| {
                    let hole = left.find(|hole| clear_of(&round, hole.at, spacing))?;
                    round.push(hole.at);
                    Some((*pocket, hole))
                })
                .collect();
            if tries.is_empty() {
                break;
            }
            let drilled: Vec<DrainHole> =
                chosen.iter().chain(&tries).map(|(_, hole)| *hole).collect();
            let after = self.trapped(&drilled)?;
            settle(pockets, &mut chosen, tries, &after);
            if chosen.len() == wanted.len() {
                break;
            }
        }
        taken.extend(mouths(&chosen));
        Ok(chosen)
    }
}

fn mouths(chosen: &[(usize, DrainHole)]) -> impl Iterator<Item = Vec3> + '_ {
    chosen.iter().map(|(_, hole)| hole.at)
}

fn clear_of(taken: &[Vec3], at: Vec3, spacing: Scalar) -> bool {
    taken.iter().all(|other| other.distance(at) >= spacing)
}

/// Keeps of `chosen` the holes whose pocket the round's scan, `after`, no longer finds, and adds
/// each of `tries` that emptied its own; a pocket a failed try opened then waits for its own.
fn settle(
    pockets: &[Trapped],
    chosen: &mut Vec<(usize, DrainHole)>,
    tries: Vec<(usize, DrainHole)>,
    after: &[Trapped],
) {
    let emptied = |pocket: usize| {
        pockets
            .get(pocket)
            .is_some_and(|pocket| !still_trapped(pocket, after))
    };
    chosen.retain(|(pocket, _)| emptied(*pocket));
    chosen.extend(tries.into_iter().filter(|(pocket, _)| emptied(*pocket)));
}

/// Whether any of `trapped` overlaps `pocket`'s box: a hole that left part of a pocket, or moved
/// its floor, leaves it there.
fn still_trapped(pocket: &Trapped, trapped: &[Trapped]) -> bool {
    trapped
        .iter()
        .any(|other| pocket.bounds.intersects(&other.bounds))
}

#[cfg(test)]
mod tests;
