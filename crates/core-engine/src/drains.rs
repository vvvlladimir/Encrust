use core_geometry::{Aabb, Bvh, Mesh, Scalar, Vec3};
use core_supports::Trapped;
use core_volume::{DrainHole, HoleSize, Toward, drill, holes_out, pierce};

use crate::bake::Baked;
use crate::error::EngineError;
use crate::trapped::trapped_in;

/// The most holes tried for one pocket in each role. A search scans the model once to find
/// the pockets, once a round for drains and for vents, and once more for what is left (ADR 0396).
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
    /// Points a mouth keeps a hole's spacing from, such as support contacts, millimetres.
    pub keep_clear: &'a [Vec3],
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
    let mut base = search.mesh.clone();
    if let Some(cuts) = search.cuts {
        base.append(cuts);
    }
    let mut scans = Scans {
        search,
        base,
        count: 0,
    };
    let pockets = scans.trapped(&[])?;
    if pockets.is_empty() {
        return Ok(Drained {
            scans: scans.count,
            ..Drained::default()
        });
    }

    let bvh = Bvh::build(search.mesh);
    let mut taken = search.keep_clear.to_vec();
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

/// The model a search scans, and how often it has.
struct Scans<'a> {
    search: &'a DrainSearch<'a>,
    /// The model with the cuts it already had.
    base: Mesh,
    count: usize,
}

impl Scans<'_> {
    /// What is trapped with `holes` drilled besides the cuts the model had.
    fn trapped(&mut self, holes: &[DrainHole]) -> Result<Vec<Trapped>, EngineError> {
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
        self.count += 1;
        let found = trapped_in(&baked, self.search.layer_height_mm, |_, _| true)?;
        Ok(found.unwrap_or_default())
    }

    /// The holes into `pocket` looked for `toward`, best first, carried through the wall.
    fn proposals(&self, pocket: &Trapped, toward: Toward, bvh: &Bvh) -> Vec<DrainHole> {
        let search = self.search;
        let found = holes_out(
            search.mesh,
            bvh,
            pocket.at,
            toward,
            search.size,
            search.plate_z,
        );
        match search.wall {
            Some((thickness_mm, voxel_mm)) => pierce(&found, thickness_mm, voxel_mm),
            None => found,
        }
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
    ) -> Result<Vec<(usize, DrainHole)>, EngineError> {
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
            let mut round = taken.clone();
            let tries: Vec<(usize, DrainHole)> = candidates
                .iter_mut()
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
            for (pocket, hole) in tries {
                if !still_trapped(&pockets[pocket], &after) {
                    chosen.push((pocket, hole));
                    taken.push(hole.at);
                    candidates.retain(|(waiting, _)| *waiting != pocket);
                }
            }
            if candidates.is_empty() {
                break;
            }
        }
        Ok(chosen)
    }
}

fn clear_of(taken: &[Vec3], at: Vec3, spacing: Scalar) -> bool {
    taken.iter().all(|other| other.distance(at) >= spacing)
}

/// Whether `pocket` is among `trapped`: one whose box holds the point it was found at, since a
/// hole that failed changes a pocket's box a little and not where its floor is.
fn still_trapped(pocket: &Trapped, trapped: &[Trapped]) -> bool {
    trapped.iter().any(|other| holds(&other.bounds, pocket.at))
}

fn holds(bounds: &Aabb, at: Vec3) -> bool {
    at.cmpge(bounds.mins).all() && at.cmple(bounds.maxs).all()
}

#[cfg(test)]
mod tests;
