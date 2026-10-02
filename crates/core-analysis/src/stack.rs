use std::collections::BTreeMap;
use std::f64::consts::PI;

use crate::contact::{Contact, MIN_ISLAND_MM2, contacts};
use crate::layer::{Cured, LayerMeasure, Piece, Stretch};

/// Peel force per unit of torsion constant, N/mm^4: Stefan's `3 mu h' / h^3`, set so a
/// 100 cm^2 disc pulls about 100 N, the order users report.
pub const PEEL_N_PER_MM4: f64 = 6.4e-6;
/// Stress a neck of green resin is taken to fail at, megapascals. Not measured: a warning
/// level.
pub const NECK_LIMIT_MPA: f64 = 5.0;
/// Pull on the film past which a layer tears or lifts the model off the plate, newtons:
/// about 3 kg, a solid section some 80 mm across. A warning level, like the neck's.
pub const PEEL_LIMIT_N: f64 = 30.0;
/// A contact narrower than this share of the neck under it becomes the new neck.
const NARROWER: f64 = 0.9;

/// A layer worth naming, and the reading that named it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    /// Index into the stack, counted from the plate.
    pub layer: usize,
    pub value: f32,
}

/// Where a print is likely to fail, and why.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Risk {
    /// Index into the stack of the layer it happens on, counted from the plate.
    pub layer: usize,
    /// Where on the layer, panel millimetres from the top-left pixel.
    pub at_mm: [f32; 2],
    pub kind: RiskKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskKind {
    /// Cured over nothing: it sticks to the film and stays in the vat.
    Island { area_mm2: f32 },
    /// Pulled hard enough, far enough from the narrowest neck under it, to break it.
    Lever {
        stress_mpa: f32,
        lever_mm: f32,
        neck_mm2: f32,
    },
    /// A layer whose suction on the film is past `PEEL_LIMIT_N`; a run of them is named
    /// once, at its hardest.
    Peel { force_n: f32 },
}

/// What a stack cures, folded a layer at a time in print order. It holds the layer below
/// and one reading a layer, never a mask.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measured {
    /// Layers glued to the plate, which the plate holds rather than the model, so they
    /// are never named.
    held: usize,
    /// Whether an island is taken out of its layer rather than named.
    removing: bool,
    layers: Vec<LayerMeasure>,
    volume_mm3: f64,
    hardest_pull: Option<Peak>,
    largest_growth: Option<Peak>,
    below: Option<(Cured, Vec<Carried>)>,
    islands: Vec<Risk>,
    /// The worst lever on each neck, so a long arm is named once rather than a layer at a
    /// time.
    levers: BTreeMap<u32, Risk>,
    necks: u32,
    peels: Vec<Risk>,
    /// The run of layers past the peel limit still going on.
    peeling: Option<Risk>,
    /// What was taken out of each layer that lost an island, in print order.
    removed: Vec<(usize, Vec<Stretch>)>,
    removed_pieces: usize,
}

/// What a piece of the layer below hands up: whether it reaches the plate, and the
/// narrowest section between it and the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Carried {
    anchored: bool,
    neck: Neck,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Neck {
    area_mm2: f64,
    at_mm: [f64; 2],
    id: u32,
}

impl Measured {
    /// A fold over a stack whose first `held_layers` are the bottom block.
    pub fn new(held_layers: usize) -> Self {
        Self {
            held: held_layers,
            ..Self::default()
        }
    }

    /// The same fold, taking every island out of its layer instead of naming it. What
    /// stood only on an island becomes one in turn, so a floating part goes whole.
    #[must_use]
    pub fn removing_islands(mut self) -> Self {
        self.removing = true;
        self
    }

    /// Folds the next layer in, `thickness_mm` thick, and hands back what was taken out
    /// of it, which is nothing unless islands are being removed.
    pub fn push(&mut self, layer: Cured, thickness_mm: f32) -> Vec<Stretch> {
        let index = self.layers.len();
        let (layer, carried, taken) = match self.below.take() {
            None => {
                let carried = self.on_plate(&layer);
                (layer, carried, Vec::new())
            }
            Some((below, under)) => {
                let (layer, taken) = if self.removing {
                    self.clear(layer, &below)
                } else {
                    (layer, Vec::new())
                };
                let carried = self.carry(index, &layer, &below, &under);
                (layer, carried, taken)
            }
        };
        self.tally(index, &layer, thickness_mm);
        if !taken.is_empty() {
            self.removed.push((index, taken.clone()));
        }
        self.below = Some((layer, carried));
        taken
    }

    /// The fold with the layer below let go, once no more layers are coming.
    #[must_use]
    pub fn finish(mut self) -> Self {
        self.below = None;
        self
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Area, pull and pieces of every layer so far, in print order.
    pub fn layers(&self) -> &[LayerMeasure] {
        &self.layers
    }

    /// Resin the cured pixels come to, cubic millimetres.
    pub fn volume_mm3(&self) -> f32 {
        self.volume_mm3 as f32
    }

    /// The layer that pulls hardest on the film, in mm^4, above the bottom block.
    pub fn hardest_pull(&self) -> Option<Peak> {
        self.hardest_pull
    }

    /// The layer whose area grows most over the one under it, in mm^2, above the bottom
    /// block. `None` when no layer there grows.
    pub fn largest_growth(&self) -> Option<Peak> {
        self.largest_growth
    }

    /// Pieces taken out as islands, counting each layer of a floating part.
    pub fn removed_islands(&self) -> usize {
        self.removed_pieces
    }

    /// What was taken out of `layer`, in reading order.
    pub fn removed_from(&self, layer: usize) -> &[Stretch] {
        self.removed
            .binary_search_by_key(&layer, |(index, _)| *index)
            .map_or(&[], |found| &self.removed[found].1)
    }

    /// Everything likely to fail, in print order.
    pub fn risks(&self) -> Vec<Risk> {
        let mut risks: Vec<Risk> = self.islands.clone();
        risks.extend(self.levers.values().copied());
        risks.extend(self.peels.iter().chain(&self.peeling).copied());
        risks.sort_by_key(|risk| risk.layer);
        risks
    }

    /// The layer the print most likely fails on: the first island, since nothing cured
    /// over nothing survives, or else the neck under the most stress, or else the hardest
    /// peel.
    pub fn worst(&self) -> Option<Risk> {
        let hardest = |risks: &mut dyn Iterator<Item = Risk>| {
            risks.max_by(|a, b| severity(a).total_cmp(&severity(b)))
        };
        self.islands
            .first()
            .copied()
            .or_else(|| hardest(&mut self.levers.values().copied()))
            .or_else(|| hardest(&mut self.peels.iter().chain(&self.peeling).copied()))
    }

    /// Takes every piece of `layer` that touches nothing in `below` out of it.
    fn clear(&mut self, layer: Cured, below: &Cured) -> (Cured, Vec<Stretch>) {
        let mut dropped = vec![true; layer.pieces().len()];
        for contact in contacts(&layer, below) {
            dropped[contact.above as usize] = false;
        }
        let count = dropped.iter().filter(|dropped| **dropped).count();
        if count == 0 {
            return (layer, Vec::new());
        }
        self.removed_pieces += count;
        layer.without(&dropped)
    }

    /// The layer's own readings: its area and volume, and whether it pulls too hard.
    fn tally(&mut self, index: usize, layer: &Cured, thickness_mm: f32) {
        let measure = layer.measure();
        let growth = measure.area_mm2 - self.layers.last().map_or(0.0, |last| last.area_mm2);
        self.layers.push(measure);
        self.volume_mm3 += f64::from(measure.area_mm2) * f64::from(thickness_mm);
        if index < self.held {
            return;
        }
        raise(&mut self.hardest_pull, index, measure.peel_mm4);
        raise(&mut self.largest_growth, index, growth);

        let force_n = f64::from(measure.peel_mm4) * PEEL_N_PER_MM4;
        if force_n < PEEL_LIMIT_N {
            self.peels.extend(self.peeling.take());
            return;
        }
        let risk = Risk {
            layer: index,
            at_mm: centre(layer),
            kind: RiskKind::Peel {
                force_n: force_n as f32,
            },
        };
        if self
            .peeling
            .is_none_or(|run| severity(&risk) > severity(&run))
        {
            self.peeling = Some(risk);
        }
    }

    fn on_plate(&mut self, layer: &Cured) -> Vec<Carried> {
        let pieces = layer.pieces().to_vec();
        pieces
            .iter()
            .map(|piece| Carried {
                anchored: true,
                neck: self.neck(f64::from(piece.area_mm2), piece.centre_mm.map(f64::from)),
            })
            .collect()
    }

    fn carry(
        &mut self,
        index: usize,
        layer: &Cured,
        below: &Cured,
        under: &[Carried],
    ) -> Vec<Carried> {
        let pixel_mm2 = f64::from(layer.pitch().x) * f64::from(layer.pitch().y);
        let pitch = [f64::from(layer.pitch().x), f64::from(layer.pitch().y)];
        let found = contacts(layer, below);
        let mut next = 0;
        let mut carried = Vec::with_capacity(layer.pieces().len());
        for (piece_index, piece) in layer.pieces().iter().enumerate() {
            let start = next;
            while next < found.len() && found[next].above as usize == piece_index {
                next += 1;
            }
            let touching = &found[start..next];
            carried.push(self.settle(index, piece, touching, under, pixel_mm2, pitch));
        }
        carried
    }

    /// What one piece stands on: nothing, something floating, or a neck down to the plate.
    fn settle(
        &mut self,
        index: usize,
        piece: &Piece,
        touching: &[Contact],
        under: &[Carried],
        pixel_mm2: f64,
        pitch: [f64; 2],
    ) -> Carried {
        let floating = Carried {
            anchored: false,
            neck: Neck {
                area_mm2: 0.0,
                at_mm: [0.0; 2],
                id: u32::MAX,
            },
        };
        if touching.is_empty() {
            if piece.area_mm2 >= MIN_ISLAND_MM2 {
                self.islands.push(Risk {
                    layer: index,
                    at_mm: piece.centre_mm,
                    kind: RiskKind::Island {
                        area_mm2: piece.area_mm2,
                    },
                });
            }
            return floating;
        }
        let held: Vec<&Contact> = touching
            .iter()
            .filter(|contact| under[contact.below as usize].anchored)
            .collect();
        if held.is_empty() {
            return floating;
        }

        let neck = self.narrowest(&held, under, pixel_mm2, pitch);
        if index >= self.held {
            self.weigh(index, piece, neck);
        }
        Carried {
            anchored: true,
            neck,
        }
    }

    /// The narrower of what this piece touches and the necks under what it touches.
    fn narrowest(
        &mut self,
        held: &[&Contact],
        under: &[Carried],
        pixel_mm2: f64,
        pitch: [f64; 2],
    ) -> Neck {
        let pixels: u64 = held.iter().map(|contact| contact.pixels).sum();
        let contact_mm2 = pixels as f64 * pixel_mm2;
        let necks = held
            .iter()
            .map(|contact| under[contact.below as usize].neck);
        let (mut neck_mm2, mut x, mut y) = (0.0, 0.0, 0.0);
        let mut widest: Option<Neck> = None;
        for neck in necks {
            neck_mm2 += neck.area_mm2;
            x += neck.at_mm[0] * neck.area_mm2;
            y += neck.at_mm[1] * neck.area_mm2;
            if widest.is_none_or(|widest| neck.area_mm2 > widest.area_mm2) {
                widest = Some(neck);
            }
        }

        if contact_mm2 < NARROWER * neck_mm2 || neck_mm2 <= 0.0 {
            let (sx, sy): (f64, f64) = held
                .iter()
                .fold((0.0, 0.0), |(x, y), c| (x + c.x_px, y + c.y_px));
            let at = [sx / pixels as f64 * pitch[0], sy / pixels as f64 * pitch[1]];
            return self.neck(contact_mm2, at);
        }
        Neck {
            area_mm2: neck_mm2,
            at_mm: [x / neck_mm2, y / neck_mm2],
            id: widest.map_or(u32::MAX, |neck| neck.id),
        }
    }

    /// The stress this layer's pull puts on the neck under it: tension over its section,
    /// and the bending of the pull held off to one side. See `docs/design/analysis.md`.
    fn weigh(&mut self, index: usize, piece: &Piece, neck: Neck) {
        let force_n = f64::from(piece.peel_mm4) * PEEL_N_PER_MM4;
        let radius_mm = (neck.area_mm2 / PI).sqrt();
        let [cx, cy] = piece.centre_mm.map(f64::from);
        let lever_mm = (cx - neck.at_mm[0]).hypot(cy - neck.at_mm[1]);
        let section_modulus = PI * radius_mm.powi(3) / 4.0;
        let stress_mpa = force_n / neck.area_mm2 + force_n * lever_mm / section_modulus;
        if stress_mpa < NECK_LIMIT_MPA {
            return;
        }
        let risk = Risk {
            layer: index,
            at_mm: piece.centre_mm,
            kind: RiskKind::Lever {
                stress_mpa: stress_mpa as f32,
                lever_mm: lever_mm as f32,
                neck_mm2: neck.area_mm2 as f32,
            },
        };
        let worse = |held: &Risk| severity(&risk) > severity(held);
        if self.levers.get(&neck.id).is_none_or(worse) {
            self.levers.insert(neck.id, risk);
        }
    }

    fn neck(&mut self, area_mm2: f64, at_mm: [f64; 2]) -> Neck {
        self.necks += 1;
        Neck {
            area_mm2,
            at_mm,
            id: self.necks,
        }
    }
}

/// How bad a risk is against others of its kind.
fn severity(risk: &Risk) -> f32 {
    match risk.kind {
        RiskKind::Island { .. } => f32::INFINITY,
        RiskKind::Lever { stress_mpa, .. } => stress_mpa,
        RiskKind::Peel { force_n } => force_n,
    }
}

/// The middle of everything a layer cures, weighted by area.
fn centre(layer: &Cured) -> [f32; 2] {
    let (mut area, mut x, mut y) = (0.0, 0.0, 0.0);
    for piece in layer.pieces() {
        area += piece.area_mm2;
        x += piece.centre_mm[0] * piece.area_mm2;
        y += piece.centre_mm[1] * piece.area_mm2;
    }
    if area > 0.0 {
        [x / area, y / area]
    } else {
        [0.0; 2]
    }
}

fn raise(peak: &mut Option<Peak>, layer: usize, value: f32) {
    if value > 0.0 && peak.is_none_or(|peak| value > peak.value) {
        *peak = Some(Peak { layer, value });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::cure;
    use crate::layer::tests::paint;
    use core_raster::PixelPitch;

    /// Half a millimetre a pixel, so a 60 pixel panel holds parts heavy enough to pull.
    const PITCH: PixelPitch = PixelPitch { x: 0.5, y: 0.5 };

    fn fold(held: usize, layers: &[Cured]) -> Measured {
        let mut measured = Measured::new(held);
        for layer in layers {
            measured.push(layer.clone(), 0.05);
        }
        measured
    }

    fn layer(lit: impl Fn(u32, u32) -> bool) -> Cured {
        cure(&paint(60, 60, |x, y| u8::from(lit(x, y)) * 255), PITCH)
    }

    fn square(x0: u32, y0: u32, side: u32) -> Cured {
        layer(move |x, y| (x0..x0 + side).contains(&x) && (y0..y0 + side).contains(&y))
    }

    #[test]
    fn the_volume_is_each_layer_at_its_own_thickness() {
        let mut measured = Measured::new(0);
        measured.push(square(0, 0, 40), 0.05);
        measured.push(square(0, 0, 40), 0.1);
        // 40 px at 0.5 mm is 20 mm a side.
        assert!((measured.volume_mm3() - 400.0 * 0.15).abs() < 1e-3);
        assert_eq!(measured.layer_count(), 2);
    }

    #[test]
    fn the_hardest_pull_and_the_largest_growth_name_their_layers() {
        let strip = layer(|_, y| y < 2);
        let measured = fold(0, &[strip, square(0, 0, 40), square(0, 0, 45)]);
        assert_eq!(measured.hardest_pull().map(|peak| peak.layer), Some(2));
        assert_eq!(measured.largest_growth().map(|peak| peak.layer), Some(1));
    }

    #[test]
    fn the_bottom_block_is_never_named() {
        let measured = fold(2, &[square(0, 0, 40), square(0, 0, 40), square(0, 0, 10)]);
        assert_eq!(measured.hardest_pull().map(|peak| peak.layer), Some(2));
        assert_eq!(
            measured.largest_growth(),
            None,
            "the stack only shrinks after it"
        );
    }

    #[test]
    fn a_piece_that_starts_over_nothing_is_named_once_as_an_island() {
        let base = square(0, 0, 10);
        let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
        let measured = fold(0, &[base, both.clone(), both]);
        let risks = measured.risks();
        assert_eq!(risks.len(), 1, "{risks:?}");
        assert_eq!(risks[0].layer, 1);
        assert!(matches!(risks[0].kind, RiskKind::Island { .. }));
        assert_eq!(measured.worst().map(|risk| risk.layer), Some(1));
    }

    #[test]
    fn a_column_standing_straight_carries_no_lever() {
        let column: Vec<Cured> = (0..6).map(|_| square(20, 20, 20)).collect();
        assert!(fold(0, &column).risks().is_empty());
    }

    #[test]
    fn a_wide_slab_off_to_one_side_of_a_thin_neck_is_a_lever() {
        // A pin one pixel across in the corner, and a 20 mm slab hanging off it.
        let pin = square(0, 0, 1);
        let slab = square(0, 0, 40);
        let measured = fold(0, &[pin.clone(), pin, slab]);
        let risks = measured.risks();
        assert_eq!(risks.len(), 1, "{risks:?}");
        let RiskKind::Lever {
            lever_mm, neck_mm2, ..
        } = risks[0].kind
        else {
            panic!("a lever, not {:?}", risks[0].kind);
        };
        assert_eq!(risks[0].layer, 2);
        assert!(
            (neck_mm2 - 0.25).abs() < 1e-4,
            "the pin is the neck: {neck_mm2}"
        );
        // From the pin's centre at 0.25 mm to the slab's at 10 mm, on both axes.
        assert!(
            (lever_mm - 9.75 * std::f32::consts::SQRT_2).abs() < 1e-3,
            "the slab's centre is far off the pin: {lever_mm}"
        );
    }

    #[test]
    fn removing_islands_takes_a_floating_part_out_whole_and_names_nothing() {
        let base = square(0, 0, 10);
        let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
        let mut measured = Measured::new(0).removing_islands();
        let taken: Vec<usize> = [base, both.clone(), both]
            .into_iter()
            .map(|layer| measured.push(layer, 0.05).len())
            .collect();
        // The second part is 30 rows tall, on each of the two layers it has.
        assert_eq!(taken, vec![0, 30, 30]);
        assert_eq!(
            measured.removed_islands(),
            2,
            "its first layer and the one on it"
        );
        assert!(measured.risks().is_empty());
        assert_eq!(measured.removed_from(2).len(), 30);
        assert!(measured.removed_from(0).is_empty());
        // Only the base is left: 5 x 5 mm on three layers.
        assert!((measured.volume_mm3() - 25.0 * 0.15).abs() < 1e-3);
    }

    #[test]
    fn a_run_of_layers_pulling_past_the_limit_is_named_once_at_its_hardest() {
        // A 30 mm square pulls about 5 N; a pitch of 3 mm makes the same pixels 180 mm.
        let big = PixelPitch { x: 3.0, y: 3.0 };
        let wide = |side: u32| {
            cure(
                &paint(60, 60, |x, y| u8::from(x < side && y < side) * 255),
                big,
            )
        };
        let mut measured = Measured::new(0);
        for side in [10, 40, 60, 40, 10, 50] {
            measured.push(wide(side), 0.05);
        }
        let peels: Vec<usize> = measured
            .risks()
            .iter()
            .filter(|risk| matches!(risk.kind, RiskKind::Peel { .. }))
            .map(|risk| risk.layer)
            .collect();
        assert_eq!(
            peels,
            vec![2, 5],
            "one run peaking at layer 2, and layer 5 alone"
        );
    }

    #[test]
    fn the_same_slab_centred_on_a_broad_neck_is_not() {
        let post = square(15, 15, 30);
        let slab = square(5, 5, 50);
        assert!(fold(0, &[post.clone(), post, slab]).risks().is_empty());
    }
}
