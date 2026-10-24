use super::STEADY;
use super::held::Held;
use crate::config::{PLATE_CONTACT_MM, SampleConfig};
use crate::field::{Field, Grid, Parts};
use crate::low::Lows;
use core_geometry::{Scalar, Vec2, Vec3};

/// The supports standing under each piece of the layer reached; a piece inherits those under
/// what it grew from (ADR 0081), kept in plate buckets one coverage radius across.
pub(super) struct Standing {
    /// One set per piece of the layer last climbed to, numbered as `Parts` numbers them.
    pub(super) held: Vec<Held>,
    /// How many supports stand under each piece or what it grew out of, counted up to [`STEADY`];
    /// kept apart from `held`, which forgets a support left far enough below.
    pub(super) tips: Vec<u8>,
    /// Where each of those pieces began, at a low point or as a speck, while nothing
    /// holds it yet.
    pub(super) hung: Vec<Option<Hung>>,
    /// The model's low points the climb has still to match to a piece.
    pub(super) lows: Lows,
    /// The pieces of the layer below, so a piece can be followed onto what it grew from.
    pub(super) below: Option<Parts>,
    /// Supports handed to the run that the climb has not reached yet, highest last.
    pub(super) pending: Vec<(Vec3, [i32; 2])>,
    /// Supports the climb has reached whose piece has not appeared at their cell yet.
    pub(super) waiting: Vec<(Vec3, [i32; 2])>,
    /// Side of one bucket, millimetres: the widest radius over which a support holds.
    pub(super) radius_mm: Scalar,
    /// The radius a support holds over when nothing says otherwise, millimetres.
    pub(super) reach_mm: Scalar,
    /// How far above a place a support may stand and still hold it: a contact sits a layer under
    /// its surface and a seed on it, so they differ by that much.
    pub(super) above_mm: Scalar,
    /// The resin one cell of one layer holds, cubic millimetres.
    pub(super) cell_mm3: Scalar,
}

/// Where a piece nothing holds began, and how much resin has grown out of it since.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hung {
    pub(super) point: Vec3,
    pub(super) volume_mm3: Scalar,
}

impl Standing {
    pub(super) fn new(
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
    pub(super) fn parents_of(&self, parts: &Parts) -> Option<Vec<Vec<u32>>> {
        self.below.as_ref().map(|below| parts.parents(below))
    }

    /// Carries what stands onto the pieces of the next layer, contacts at `z`, and takes on the
    /// seeds and low points it reaches; supports too far below are dropped, bounding memory.
    pub(super) fn climb(
        &mut self,
        parts: &Parts,
        parents: Option<Vec<Vec<u32>>>,
        cells: &[u64],
        z: Scalar,
    ) {
        let on_plate = if z <= PLATE_CONTACT_MM { STEADY } else { 0 };
        if let Some(parents) = parents {
            self.hung = self.hung_after(&parents);
            self.tips = self.tips_after(&parents, on_plate);
            self.grow_hung(cells);
            self.held = self.held_after(&parents, z);
        } else {
            self.held = vec![Held::new(self.radius_mm); parts.count()];
            self.tips = vec![on_plate; parts.count()];
            self.hung = vec![None; parts.count()];
        }
        self.take_seeds(parts, z);
        self.take_lows(parts, z);
    }

    /// Where each new piece hangs from: the first hung piece it grew out of, carrying the
    /// resin of all of them.
    fn hung_after(&self, parents: &[Vec<u32>]) -> Vec<Option<Hung>> {
        parents
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
            .collect()
    }

    fn tips_after(&self, parents: &[Vec<u32>], on_plate: u8) -> Vec<u8> {
        parents
            .iter()
            .map(|grown_from| {
                grown_from
                    .iter()
                    .fold(on_plate, |tips, parent| {
                        tips.saturating_add(self.tips[*parent as usize])
                    })
                    .min(STEADY)
            })
            .collect()
    }

    /// What stands under each new piece. The only child of its only parent takes that
    /// parent's set over, which is the whole of a wall going straight up.
    fn held_after(&mut self, parents: &[Vec<u32>], z: Scalar) -> Vec<Held> {
        let mut children = vec![0u32; self.held.len()];
        for parent in parents.iter().flatten() {
            children[*parent as usize] += 1;
        }
        parents
            .iter()
            .map(|grown_from| {
                let mut piece = self.joined(grown_from, &children, z);
                piece.buckets.shrink_to_fit();
                piece
            })
            .collect()
    }

    /// The set of a piece grown out of `grown_from`: the largest set no other piece grows
    /// out of is taken over and the rest poured into it, rather than every set copied.
    fn joined(&mut self, grown_from: &[u32], children: &[u32], z: Scalar) -> Held {
        let largest = grown_from
            .iter()
            .filter(|parent| children[**parent as usize] == 1)
            .max_by_key(|parent| self.held[**parent as usize].buckets.len())
            .copied();
        let mut piece = match largest {
            Some(largest) => self.take_held(largest),
            None => Held::new(self.radius_mm),
        };
        piece.prune(z);
        for parent in grown_from.iter().filter(|parent| Some(**parent) != largest) {
            piece.take_in(&self.held[*parent as usize], z);
        }
        piece
    }

    /// Gives every low point the climb has reached to the piece that begins at it, where
    /// nothing holds that piece yet.
    pub(super) fn take_lows(&mut self, parts: &Parts, z: Scalar) {
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
    pub(super) fn grow_hung(&mut self, cells: &[u64]) {
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
    pub(super) fn hung(&self, piece: u32) -> Option<Hung> {
        self.hung.get(piece as usize).copied().flatten()
    }

    /// Hangs `piece`, a speck nothing holds, from `point`, with the resin of its first
    /// layer.
    pub(super) fn speck(&mut self, piece: u32, point: Vec3, volume_mm3: Scalar) {
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
    pub(super) fn hang(&mut self, piece: u32, tip: Vec3) -> Vec3 {
        self.held[piece as usize].add(tip, self.reach_mm);
        self.count(piece);
        tip
    }

    /// Takes on every support handed to the run that the climb has reached; a seed sits a layer
    /// under its surface, so it waits for its piece rather than being lost and doubled.
    pub(super) fn take_seeds(&mut self, parts: &Parts, z: Scalar) {
        while let Some(seed) = self.pending.pop_if(|seed| seed.0.z <= z + self.above_mm) {
            self.waiting.push(seed);
        }
        let floor_z = z - self.radius_mm;
        let mut waiting = std::mem::take(&mut self.waiting);
        waiting.retain(|(seed, cell)| {
            let Some(piece) = parts.at(cell[0], cell[1]) else {
                return seed.z >= floor_z;
            };
            self.held[piece as usize].add(*seed, self.reach_mm);
            // A part the hand has held is held as its owner wants it, see ADR 0095.
            self.tips[piece as usize] = STEADY;
            false
        });
        self.waiting = waiting;
    }

    /// The pieces of the layer climbed to that nothing holds, by their number.
    pub(super) fn adrift(&self) -> impl Iterator<Item = u32> + '_ {
        (0u32..)
            .zip(&self.tips)
            .filter_map(|(piece, tips)| (*tips == 0).then_some(piece))
    }

    /// The pieces held, but at fewer places than a part can stand on.
    pub(super) fn unsteady(&self) -> Vec<u32> {
        (0u32..)
            .zip(&self.tips)
            .filter_map(|(piece, tips)| (1..STEADY).contains(tips).then_some(piece))
            .collect()
    }

    pub(super) fn count(&mut self, piece: u32) {
        let tips = &mut self.tips[piece as usize];
        *tips = (*tips + 1).min(STEADY);
    }

    /// Puts a support under `piece` at `at` to steady it unless one within `clear_mm` stands there;
    /// the held ball is not asked, since a part on one support is held but not steady.
    pub(super) fn prop(
        &mut self,
        piece: u32,
        at: Vec2,
        z: Scalar,
        clear_mm: Scalar,
    ) -> Option<Vec3> {
        let held = &mut self.held[piece as usize];
        if held.near(at, clear_mm.min(self.radius_mm)) {
            return None;
        }
        let contact = at.extend(z);
        held.add(contact, self.reach_mm);
        Some(contact)
    }

    /// Stops asking after `piece`, which wanted a support where none may stand: a patch
    /// painted out is left alone, not held from further up. See `docs/decisions/0093`.
    pub(super) fn leave(&mut self, piece: Option<u32>) {
        if let Some(tips) = piece.and_then(|piece| self.tips.get_mut(piece as usize)) {
            *tips = STEADY;
        }
    }

    /// Takes what stands under `piece` of the layer below, leaving nothing in its place.
    pub(super) fn take_held(&mut self, piece: u32) -> Held {
        std::mem::replace(&mut self.held[piece as usize], Held::new(self.radius_mm))
    }

    /// Keeps the layer just sampled as what the next one grows out of.
    pub(super) fn rest_on(&mut self, parts: Parts) {
        self.below = Some(parts);
    }

    /// Puts a support at `at` under `piece` at height `z`, holding a ball of `reach_mm`, unless
    /// one under the same piece holds that place; returns the contact placed.
    pub(super) fn take(
        &mut self,
        piece: Option<u32>,
        at: Vec2,
        z: Scalar,
        reach_mm: Scalar,
    ) -> Option<Vec3> {
        let contact = at.extend(z);
        let Some(piece) = piece.filter(|piece| (*piece as usize) < self.held.len()) else {
            return Some(contact);
        };
        let held = &mut self.held[piece as usize];
        if held.holds(at, z, self.above_mm) {
            return None;
        }
        held.add(contact, reach_mm);
        self.count(piece);
        Some(contact)
    }
}

/// One cell of `piece`: the middle of its bounds where the piece covers it, else its first.
pub(super) fn middle_of(piece: &Field) -> Option<[i32; 2]> {
    let (min, max) = piece.bounds()?;
    let middle = [i32::midpoint(min[0], max[0]), i32::midpoint(min[1], max[1])];
    if piece.contains(middle[0], middle[1]) {
        return Some(middle);
    }
    let (y, row) = piece.rows().next()?;
    Some([row.first()?.x0, y])
}
