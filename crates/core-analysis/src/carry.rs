//! How hard the film pulls on each tip under a model: its stack followed up a layer at a
//! time, each layer's pull shared out through the tips holding it. See `docs/design/analysis.md`.

use crate::contact::standing_on;
use crate::layer::{Cured, Piece};
use crate::neck::NECK_LIMIT_MPA;

mod bend;
mod footing;
mod frame;
mod read;
mod section;

use bend::{Bend, Slice};
use frame::Frame;
use read::{Felt, Layer};
use section::Pull;

/// How far off a tip its section may be, millimetres: a slope leaves a layer's middle plane
/// a fraction of a layer's run from the surface.
const REACH_MM: f32 = 0.15;
/// How far above itself a tip goes on looking for material, millimetres: a part that begins
/// at a point has no section at the tip's own height.
const SEAT_RISE_MM: f32 = 1.0;
/// Height two floats of one surface may differ by, millimetres.
const SAME_HEIGHT_MM: f32 = 1e-3;

/// Young's modulus of green resin, megapascals: the lowest a data sheet's cured modulus times
/// the green share published gives (ADR 0455).
pub const GREEN_MODULUS_MPA: f64 = 600.0;
/// Weight of cured resin in air, newtons a cubic millimetre: 1.15 g/cm³, no lift from the vat.
pub const CURED_N_PER_MM3: f64 = 1.15e-3 * 9.81e-3;
/// How far a layer may move across under its pull before the next is cured off it, mm.
pub const SHIFT_LIMIT_MM: f64 = 0.1;

/// One tip as the fold reads it, in the millimetres the layers are cured in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tip {
    pub at_mm: [f32; 2],
    pub z_mm: f32,
    /// Radius where it crosses the part's surface.
    pub radius_mm: f64,
    /// How the support under it gives; `None` reads it as rigid.
    pub spring: Option<Spring>,
    /// Where its support stands on the model; `None` where it stands on the plate.
    pub stands_on: Option<Footing>,
}

/// The one contact a support standing on the model meets it by (ADR 0124).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footing {
    pub at_mm: [f32; 3],
    pub radius_mm: f64,
}

/// How the support under a tip gives as the part on it is pulled, read off its tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// N per mm the tip moves along z.
    pub axial_n_per_mm: f64,
    /// N per mm the tip moves across.
    pub lateral_n_per_mm: f64,
    /// N·mm per radian the tip's top turns.
    pub turning_n_mm_per_rad: f64,
    /// The trunk the tip stands on; the tips of one tree share it.
    pub trunk: u32,
    /// The squeeze that trunk buckles under, newtons.
    pub buckles_at_n: f64,
}

/// The worst one tip saw.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Load {
    /// Highest stress in its section, megapascals.
    pub stress_mpa: f64,
    /// The height of the layer it came on, millimetres; `None` where nothing ever pulled on
    /// the tip.
    pub z_mm: Option<f32>,
    /// Whether the tip found the part it touches.
    pub seated: bool,
    /// The furthest a layer it held moved across under its pull, mm.
    pub shift_mm: f64,
    /// Where the pull that moved it furthest acted, `[x, y, z]` mm; `None` where nothing moved.
    pub shift_at_mm: Option<[f32; 3]>,
    /// What its trunk holds in squeeze over the most it was pressed with; `None` where it
    /// was never pressed or has no spring.
    pub buckling_margin: Option<f64>,
}

impl Load {
    /// What the tip holds over what pulled on it, against [`NECK_LIMIT_MPA`]: below 1 it
    /// tears; `None` where nothing pulled.
    pub fn margin(&self) -> Option<f64> {
        self.z_mm?;
        (self.stress_mpa > 0.0).then(|| NECK_LIMIT_MPA / self.stress_mpa)
    }

    /// Whether a layer it held moved past [`SHIFT_LIMIT_MM`].
    pub fn sways(&self) -> bool {
        self.shift_mm > SHIFT_LIMIT_MM
    }
}

/// A body of material grown so far and the tips under it.
#[derive(Default)]
struct Part {
    tips: Vec<u32>,
    on_plate: bool,
    /// Resin cured so far, mm³, then its first moments about x, y and z, mm⁴.
    resin: [f64; 4],
    /// How it bends above its highest tip.
    bend: Bend,
}

impl Part {
    /// Adds `piece`, `rise_mm` thick at height `z_mm`, to the resin it has cured.
    fn cure(&mut self, piece: &Piece, z_mm: f64, rise_mm: f64) {
        let volume = f64::from(piece.area_mm2) * rise_mm;
        let [x, y] = piece.centre_mm.map(f64::from);
        for (sum, add) in self.resin.iter_mut().zip([1.0, x, y, z_mm]) {
            *sum += volume * add;
        }
    }

    /// Its weight, newtons, and the point it hangs from, mm.
    fn weight(&self) -> (f64, [f64; 3]) {
        let [volume, x, y, z] = self.resin;
        if volume <= 0.0 {
            return (0.0, [0.0; 3]);
        }
        (
            volume * CURED_N_PER_MM3,
            [x / volume, y / volume, z / volume],
        )
    }
}

/// A stack followed in print order for what its tips carry. It holds one layer and a list
/// of parts, never the stack.
pub struct TipLoads<'a> {
    tips: &'a [Tip],
    /// Tips by height, and how many have reached the layer being read.
    by_height: Vec<u32>,
    reached: usize,
    /// Tips reached that have not found their part yet.
    seating: Vec<u32>,
    parts: Vec<Part>,
    /// The part each became part of, itself until joined.
    joined: Vec<u32>,
    below: Option<(Cured, Vec<u32>)>,
    layer_mm: f32,
    loads: Vec<Load>,
    /// Tips standing on the model by the height of their foot, how many the stack has passed,
    /// and the part each of those found under its foot.
    by_foot: Vec<u32>,
    footed: usize,
    feet: Vec<Option<u32>>,
}

impl<'a> TipLoads<'a> {
    /// A fold over `tips`, on a stack cut `layer_mm` thick.
    pub fn new(tips: &'a [Tip], layer_mm: f32) -> Self {
        let mut by_height: Vec<u32> = (0..tips.len() as u32).collect();
        by_height.sort_by(|a, b| tips[*a as usize].z_mm.total_cmp(&tips[*b as usize].z_mm));
        let foot_z = |tip: &u32| {
            tips[*tip as usize]
                .stands_on
                .map_or(f32::MAX, |f| f.at_mm[2])
        };
        let mut by_foot: Vec<u32> = (0..tips.len() as u32)
            .filter(|tip| tips[*tip as usize].stands_on.is_some())
            .collect();
        by_foot.sort_by(|a, b| foot_z(a).total_cmp(&foot_z(b)));
        Self {
            tips,
            by_height,
            reached: 0,
            seating: Vec::new(),
            parts: Vec::new(),
            joined: Vec::new(),
            below: None,
            layer_mm,
            loads: vec![Load::default(); tips.len()],
            by_foot,
            footed: 0,
            feet: vec![None; tips.len()],
        }
    }

    /// Reads the next layer up, its middle at `z_mm`.
    pub fn push(&mut self, z_mm: f32, layer: Cured) {
        self.seat_feet(z_mm);
        let parts = self.parts_of(z_mm, &layer);
        self.seat(z_mm, &layer, &parts);
        self.weigh(z_mm, &layer, &parts);
        self.below = Some((layer, parts));
    }

    /// The part of each piece of `layer`: what it stands on joined into one, or a new one. The
    /// stack starts at the model's floor, so only a first layer that low stands on the plate.
    fn parts_of(&mut self, z_mm: f32, layer: &Cured) -> Vec<u32> {
        let mut found: Vec<Option<u32>> = vec![None; layer.pieces().len()];
        let on_plate = self.below.is_none() && z_mm <= self.layer_mm;
        if let Some((below, under)) = self.below.take() {
            for (above, piece) in standing_on(layer, &below) {
                let part = under[piece as usize];
                match found[above as usize] {
                    Some(other) => self.join(other, part),
                    None => found[above as usize] = Some(part),
                }
            }
        }
        found
            .into_iter()
            .map(|part| {
                let part = part.unwrap_or_else(|| self.new_part(on_plate));
                self.root(part)
            })
            .collect()
    }

    /// The worst each tip saw, in the order they were given.
    pub fn loads(self) -> Vec<Load> {
        self.loads
    }

    fn new_part(&mut self, on_plate: bool) -> u32 {
        let part = self.parts.len() as u32;
        self.parts.push(Part {
            on_plate,
            ..Part::default()
        });
        self.joined.push(part);
        part
    }

    fn root(&mut self, mut part: u32) -> u32 {
        while self.joined[part as usize] != part {
            let next = self.joined[part as usize];
            self.joined[part as usize] = self.joined[next as usize];
            part = next;
        }
        part
    }

    /// Joins two parts, the one with fewer tips poured into the other. What the joined part
    /// bends over starts again: the two were held apart until here.
    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.root(a), self.root(b));
        if a == b {
            return;
        }
        let fewer = self.parts[a as usize].tips.len() < self.parts[b as usize].tips.len();
        let (kept, poured) = if fewer { (b, a) } else { (a, b) };
        let poured_part = std::mem::take(&mut self.parts[poured as usize]);
        let part = &mut self.parts[kept as usize];
        part.tips.extend(poured_part.tips);
        part.on_plate |= poured_part.on_plate;
        for (sum, more) in part.resin.iter_mut().zip(poured_part.resin) {
            *sum += more;
        }
        part.bend = Bend::default();
        self.joined[poured as usize] = kept;
    }

    /// Puts each tip the layer has reached on the piece it touches, leaving those with no
    /// section near them yet for the next layer. A part bends only above its highest tip.
    fn seat(&mut self, z_mm: f32, layer: &Cured, parts: &[u32]) {
        while let Some(&tip) = self.by_height.get(self.reached) {
            if self.tips[tip as usize].z_mm > z_mm + SAME_HEIGHT_MM {
                break;
            }
            self.seating.push(tip);
            self.reached += 1;
        }
        let waiting = std::mem::take(&mut self.seating);
        for tip in waiting {
            let at = self.tips[tip as usize];
            if let Some(piece) = layer.piece_near(at.at_mm, REACH_MM) {
                let part = &mut self.parts[parts[piece as usize] as usize];
                part.tips.push(tip);
                part.bend = Bend::default();
                self.loads[tip as usize].seated = true;
            } else if z_mm < at.z_mm + SEAT_RISE_MM {
                self.seating.push(tip);
            }
        }
    }

    /// Shares each part's pull on this layer, and its weight, out through its tips; then
    /// reads again each part a support standing on it passed some of that to.
    fn weigh(&mut self, z_mm: f32, layer: &Cured, parts: &[u32]) {
        let mut pieces: Vec<(u32, usize)> = parts.iter().copied().zip(0..).collect();
        pieces.sort_unstable_by_key(|(part, _)| *part);
        let (z, rise) = (f64::from(z_mm), f64::from(self.layer_mm));
        let mut frames = Vec::new();
        let (mut read, mut passed, mut slices) = (Vec::new(), Vec::new(), Vec::new());
        for run in pieces.chunk_by(|a, b| a.0 == b.0) {
            let at = run[0].0;
            let own = || run.iter().map(|&(_, piece)| &layer.pieces()[piece]);
            own().for_each(|piece| self.parts[at as usize].cure(piece, z, rise));
            slices.extend(Slice::of(own()).map(|slice| (at, slice)));
            if self.parts[at as usize].on_plate || self.parts[at as usize].tips.is_empty() {
                continue;
            }
            let mut pull = Pull::default();
            own().for_each(|piece| pull.add(piece));
            let this = Layer {
                pull,
                weight: self.parts[at as usize].weight(),
                passed: Vec::new(),
                z_mm: z,
            };
            passed.extend(self.read_part(at, &this, &mut frames, z_mm));
            read.push((at, this));
        }
        self.pass_down(passed, &read, (z, z_mm), &mut frames);
        for (at, slice) in slices {
            self.parts[at as usize].bend.add(z, rise, &slice);
        }
    }

    /// Reads each part `passed` names again with what supports standing on it pass down; what
    /// it passes on in turn is left out.
    fn pass_down(
        &mut self,
        passed: Vec<(u32, [f64; 3], [f64; 3])>,
        read: &[(u32, Layer)],
        (z, z_mm): (f64, f32),
        frames: &mut Vec<(u32, Option<Frame>)>,
    ) {
        let mut onto: Vec<(u32, [f64; 3], [f64; 3])> = passed
            .into_iter()
            .map(|(part, force, at)| (self.root(part), force, at))
            .collect();
        onto.sort_unstable_by_key(|(part, _, _)| *part);
        for run in onto.chunk_by(|a, b| a.0 == b.0) {
            let part = run[0].0;
            if self.parts[part as usize].on_plate || self.parts[part as usize].tips.is_empty() {
                continue;
            }
            let mut this = read.iter().find(|(at, _)| *at == part).map_or_else(
                || Layer {
                    pull: Pull::default(),
                    weight: self.parts[part as usize].weight(),
                    passed: Vec::new(),
                    z_mm: z,
                },
                |(_, this)| this.clone(),
            );
            this.passed = run.iter().map(|&(_, force, at)| (force, at)).collect();
            self.read_part(part, &this, frames, z_mm);
        }
    }

    /// Reads `layer` through `part`'s tips and notes what each felt; answers what they pass
    /// down to the parts they stand on.
    fn read_part(
        &mut self,
        part: u32,
        layer: &Layer,
        frames: &mut Vec<(u32, Option<Frame>)>,
        z_mm: f32,
    ) -> Vec<(u32, [f64; 3], [f64; 3])> {
        let holds = self.holds(part, frames);
        let ids = &self.parts[part as usize].tips;
        let tips: Vec<&Tip> = ids.iter().map(|tip| &self.tips[*tip as usize]).collect();
        let reading = read::read(&tips, &holds, layer, &self.parts[part as usize].bend);
        for (&tip, felt) in ids.iter().zip(reading.felt) {
            note(
                &mut self.loads[tip as usize],
                &self.tips[tip as usize],
                felt,
                z_mm,
            );
        }
        reading.passed
    }
}

/// Keeps what `felt` adds to the worst `load` saw.
fn note(load: &mut Load, tip: &Tip, felt: Felt, z_mm: f32) {
    if felt.stress_mpa > load.stress_mpa {
        load.stress_mpa = felt.stress_mpa;
        load.z_mm = Some(z_mm);
    }
    if felt.shift_mm > load.shift_mm {
        load.shift_mm = felt.shift_mm;
        let [x, y] = felt.shift_at_mm.map(|mm| mm as f32);
        load.shift_at_mm = Some([x, y, z_mm]);
    }
    if let Some(spring) = tip.spring.filter(|_| felt.squeeze_n > 0.0) {
        let margin = spring.buckles_at_n / felt.squeeze_n;
        load.buckling_margin = Some(load.buckling_margin.map_or(margin, |was| was.min(margin)));
    }
}

#[cfg(test)]
mod tests;
