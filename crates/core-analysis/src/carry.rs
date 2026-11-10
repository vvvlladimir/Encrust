//! How hard the film pulls on each tip under a model: its stack followed up a layer at a
//! time, each layer's pull shared out through the tips holding it. See `docs/design/analysis.md`.

use crate::contact::standing_on;
use crate::layer::Cured;
use crate::neck::NECK_LIMIT_MPA;

mod section;

use section::{Pull, Section};

/// How far off a tip its section may be, millimetres: a slope leaves a layer's middle plane
/// a fraction of a layer's run from the surface.
const REACH_MM: f32 = 0.15;
/// How far above itself a tip goes on looking for material, millimetres: a part that begins
/// at a point has no section at the tip's own height.
const SEAT_RISE_MM: f32 = 1.0;
/// Height two floats of one surface may differ by, millimetres.
const SAME_HEIGHT_MM: f32 = 1e-3;

/// One tip as the fold reads it, in the millimetres the layers are cured in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tip {
    pub at_mm: [f32; 2],
    pub z_mm: f32,
    /// Radius where it crosses the part's surface.
    pub radius_mm: f64,
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
}

impl Load {
    /// What the tip holds over what pulled on it, against [`NECK_LIMIT_MPA`]: below 1 it
    /// tears; `None` where nothing pulled.
    pub fn margin(&self) -> Option<f64> {
        self.z_mm?;
        (self.stress_mpa > 0.0).then(|| NECK_LIMIT_MPA / self.stress_mpa)
    }
}

/// A body of material grown so far and the tips under it.
#[derive(Default)]
struct Part {
    tips: Vec<u32>,
    on_plate: bool,
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
}

impl<'a> TipLoads<'a> {
    /// A fold over `tips`, on a stack cut `layer_mm` thick.
    pub fn new(tips: &'a [Tip], layer_mm: f32) -> Self {
        let mut by_height: Vec<u32> = (0..tips.len() as u32).collect();
        by_height.sort_by(|a, b| tips[*a as usize].z_mm.total_cmp(&tips[*b as usize].z_mm));
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
        }
    }

    /// Reads the next layer up, its middle at `z_mm`.
    pub fn push(&mut self, z_mm: f32, layer: Cured) {
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
            tips: Vec::new(),
            on_plate,
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

    /// Joins two parts, the one with fewer tips poured into the other.
    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.root(a), self.root(b));
        if a == b {
            return;
        }
        let fewer = self.parts[a as usize].tips.len() < self.parts[b as usize].tips.len();
        let (kept, poured) = if fewer { (b, a) } else { (a, b) };
        let Part { tips, on_plate } = std::mem::take(&mut self.parts[poured as usize]);
        let part = &mut self.parts[kept as usize];
        part.tips.extend(tips);
        part.on_plate |= on_plate;
        self.joined[poured as usize] = kept;
    }

    /// Puts each tip the layer has reached on the piece it touches, leaving those with no
    /// section near them yet for the next layer.
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
                self.parts[parts[piece as usize] as usize].tips.push(tip);
                self.loads[tip as usize].seated = true;
            } else if z_mm < at.z_mm + SEAT_RISE_MM {
                self.seating.push(tip);
            }
        }
    }

    /// Shares each part's pull on this layer out through its tips.
    fn weigh(&mut self, z_mm: f32, layer: &Cured, parts: &[u32]) {
        let mut pieces: Vec<(u32, usize)> = parts.iter().copied().zip(0..).collect();
        pieces.sort_unstable_by_key(|(part, _)| *part);
        for run in pieces.chunk_by(|a, b| a.0 == b.0) {
            let part = &self.parts[run[0].0 as usize];
            if part.on_plate || part.tips.is_empty() {
                continue;
            }
            let mut pull = Pull::default();
            for &(_, piece) in run {
                pull.add(&layer.pieces()[piece]);
            }
            let section = Section::of(part.tips.iter().map(|tip| &self.tips[*tip as usize]));
            for &tip in &part.tips {
                let stress_mpa = section.stress_mpa(&self.tips[tip as usize], &pull);
                let load = &mut self.loads[tip as usize];
                if stress_mpa > load.stress_mpa {
                    load.stress_mpa = stress_mpa;
                    load.z_mm = Some(z_mm);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
