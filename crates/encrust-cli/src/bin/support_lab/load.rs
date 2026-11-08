//! How hard the film pulls on each tip: the model's own stack followed up a layer at a time,
//! each layer's pull shared out through the tips holding it. See `docs/design/analysis.md`.

use anyhow::Result;
use core_analysis::{Cured, Piece, cure, standing_on};
use core_engine::Run;
use core_geometry::{FastMap, Mesh};
use core_raster::{RasterSettings, Rasterizer, ScanlineRasterizer};
use core_slicer::Layer;
use printer_profiles::{MaterialProfile, PrinterProfile};
use rayon::prelude::*;

use crate::cure::plate_of;

/// How far off a tip its section may be, millimetres: a slope leaves a layer's middle plane
/// a fraction of a layer's run from the surface.
const REACH_MM: f32 = 0.15;
/// How far above itself a tip goes on looking for material, millimetres: a part that begins
/// at a point has no section at the tip's own height.
const SEAT_RISE_MM: f32 = 1.0;
/// Height two floats of one surface may differ by, millimetres.
const SAME_HEIGHT_MM: f32 = 1e-3;

/// One tip as the fold reads it, plate millimetres.
#[derive(Debug, Clone, Copy)]
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
    /// The height of the layer it came on, plate millimetres; `None` where nothing ever
    /// pulled on the tip.
    pub z_mm: Option<f32>,
    /// Whether the tip found the part it touches.
    pub seated: bool,
}

/// What `tips` carry as `model` alone prints on `printer`, one load a tip.
pub fn loads(
    model: &Mesh,
    tips: &[Tip],
    printer: &PrinterProfile,
    material: &MaterialProfile,
) -> Result<Vec<Load>> {
    let run = Run::measuring(&plate_of(model, &Mesh::default(), printer, material))?;
    let mut fold = Fold::new(tips, material.layer_height_mm);
    let group = rayon::current_num_threads().max(1);
    run.windows().stream(run.solid(), |sliced| -> Result<()> {
        for layers in sliced.layers.chunks(group) {
            for (layer, cured) in layers.iter().zip(cured(layers, run.panel())?) {
                fold.push(layer.z, cured);
            }
        }
        Ok(())
    })?;
    Ok(fold.loads)
}

fn cured(layers: &[Layer], panel: &RasterSettings) -> Result<Vec<Cured>> {
    layers
        .par_iter()
        .map(|layer| {
            let rastered = ScanlineRasterizer.rasterize(layer, panel)?;
            Ok(cure(&rastered.runs, panel.pitch))
        })
        .collect()
}

/// A body of material grown so far and the tips under it.
#[derive(Default)]
struct Part {
    tips: Vec<u32>,
    on_plate: bool,
}

/// A stack followed in print order. It holds one layer and a list of parts, never the stack.
struct Fold<'a> {
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

impl<'a> Fold<'a> {
    fn new(tips: &'a [Tip], layer_mm: f32) -> Self {
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

    fn push(&mut self, z_mm: f32, layer: Cured) {
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
        let mut pulls: FastMap<u32, Pull> = FastMap::default();
        for (piece, part) in layer.pieces().iter().zip(parts) {
            pulls.entry(*part).or_default().add(piece);
        }
        for (part, pull) in pulls {
            let part = &self.parts[part as usize];
            if part.on_plate || part.tips.is_empty() {
                continue;
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

/// One layer's pull on one part: its force, and that force's moments.
#[derive(Debug, Clone, Copy, Default)]
struct Pull {
    force_n: f64,
    moment_x: f64,
    moment_y: f64,
    /// Force times how far off its centre the film's last grip puts it.
    peel: f64,
}

impl Pull {
    fn add(&mut self, piece: &Piece) {
        let force = f64::from(piece.pull_n);
        self.force_n += force;
        self.moment_x += force * f64::from(piece.centre_mm[0]);
        self.moment_y += force * f64::from(piece.centre_mm[1]);
        self.peel += force * f64::from(piece.gyration_mm) / 2.0;
    }
}

/// The tips under a part read as one section of separate discs.
#[derive(Debug, Clone, Copy)]
struct Section {
    area_mm2: f64,
    centre_mm: [f64; 2],
    /// The inverse of its second moments, `[xx, xy, yy]`, per mm^4.
    inverse: [f64; 3],
    /// The inverse's largest eigenvalue, per mm^4: its weakest way to bend.
    weakest: f64,
}

impl Section {
    fn of<'t>(tips: impl Iterator<Item = &'t Tip> + Clone) -> Self {
        let area = |tip: &Tip| std::f64::consts::PI * tip.radius_mm * tip.radius_mm;
        let (mut area_mm2, mut sum_x, mut sum_y) = (0.0, 0.0, 0.0);
        for tip in tips.clone() {
            let a = area(tip);
            area_mm2 += a;
            sum_x += a * f64::from(tip.at_mm[0]);
            sum_y += a * f64::from(tip.at_mm[1]);
        }
        let centre_mm = [sum_x / area_mm2, sum_y / area_mm2];
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for tip in tips {
            let (dx, dy) = off(tip, centre_mm);
            let a = area(tip);
            let own = a * tip.radius_mm * tip.radius_mm / 4.0;
            xx += a * dx * dx + own;
            xy += a * dx * dy;
            yy += a * dy * dy + own;
        }
        let det = xx * yy - xy * xy;
        let softest = (xx + yy) / 2.0 - ((xx - yy) / 2.0).hypot(xy);
        Self {
            area_mm2,
            centre_mm,
            inverse: [yy / det, -xy / det, xx / det],
            weakest: 1.0 / softest,
        }
    }

    /// `[x, y]` through the inverse: the stress gradient a moment makes, or the reverse.
    fn turned(&self, x: f64, y: f64) -> (f64, f64) {
        let [xx, xy, yy] = self.inverse;
        (xx * x + xy * y, xy * x + yy * y)
    }

    /// The highest stress in `tip`, whichever way the film lets go: the pull's own moment
    /// about the section, then its peel lever taken the way that is worst for this tip.
    fn stress_mpa(&self, tip: &Tip, pull: &Pull) -> f64 {
        if pull.force_n <= 0.0 {
            return 0.0;
        }
        let (mx, my) = (
            pull.moment_x - pull.force_n * self.centre_mm[0],
            pull.moment_y - pull.force_n * self.centre_mm[1],
        );
        let (gx, gy) = self.turned(mx, my);
        let (dx, dy) = off(tip, self.centre_mm);
        let (tx, ty) = self.turned(dx, dy);
        let peel = pull.peel * (tx.hypot(ty) + tip.radius_mm * self.weakest);
        (pull.force_n / self.area_mm2 + gx * dx + gy * dy + gx.hypot(gy) * tip.radius_mm + peel)
            .max(0.0)
    }
}

fn off(tip: &Tip, centre_mm: [f64; 2]) -> (f64, f64) {
    (
        f64::from(tip.at_mm[0]) - centre_mm[0],
        f64::from(tip.at_mm[1]) - centre_mm[1],
    )
}

#[cfg(test)]
mod tests;
