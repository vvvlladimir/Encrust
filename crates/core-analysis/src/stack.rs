use std::collections::BTreeMap;
use std::f64::consts::PI;

use crate::contact::{Contact, MIN_ISLAND_MM2, contacts};
use crate::cups::Cups;
use crate::layer::{Cured, LayerMeasure, Piece, Stretch};
use crate::pull::PEEL_LIMIT_N;

/// Stress a neck of green resin is taken to fail at, megapascals. Not measured: a warning
/// level.
pub const NECK_LIMIT_MPA: f64 = 5.0;
/// A piece narrower than this across, millimetres, as a disc of its area: thinner than the
/// finest support tip, so a tip is never named, and too thin to survive the peel.
pub const THIN_MM: f32 = 0.15;
/// A thin run shorter than this is the point a shape starts or ends on, not a wire that
/// has to stand, millimetres.
pub const MIN_THIN_RUN_MM: f32 = 1.0;
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
    /// Stands on an island that was taken out, so it hangs over nothing as the island did;
    /// named once, on the layer over the island.
    Floating { area_mm2: f32 },
    /// Pulled hard enough, far enough from the narrowest neck under it, to break it.
    Lever {
        stress_mpa: f32,
        /// How far off the neck the pull acts, millimetres.
        lever_mm: f32,
        neck_mm2: f32,
    },
    /// A layer whose suction on the film is past `PEEL_LIMIT_N`; a run of them is named
    /// once, at its hardest.
    Peel { force_n: f32 },
    /// A hole sealed from below, by the plate or the model, that every lift pulls a vacuum
    /// in; named once, where it is widest.
    SuctionCup { area_mm2: f32, volume_mm3: f32 },
    /// A piece held from below but narrower than `THIN_MM`; a run of them at least
    /// `MIN_THIN_RUN_MM` tall is named once, where it starts.
    Thin { width_mm: f32 },
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
    /// How many layers of what stood on an island taken out go too, each one the island
    /// of the one over it, rather than being named.
    passes: u32,
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
    removed_parts: usize,
    thins: Vec<Risk>,
    floating: Vec<Risk>,
    cups: Cups,
    /// How thick the layer being folded is, millimetres.
    layer_mm: f32,
}

/// What a piece of the layer below hands up: whether it reaches the plate, the narrowest
/// section between it and the plate, and the thin run it tops, if it is too thin.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Carried {
    anchored: bool,
    neck: Neck,
    thin: Option<ThinRun>,
    /// Touches nothing below: an island, whether it is named or taken out.
    island: bool,
    /// Taken out of its layer: an island being removed at depth 0, or what hung over one
    /// at its depth above it.
    gone: Option<u32>,
}

/// Thin pieces standing on each other, followed up while they stay thin.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ThinRun {
    /// What it is named as once it is tall enough: where it starts.
    start: Risk,
    height_mm: f32,
    named: bool,
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

    /// The same fold, taking every island out of its layer instead of naming it. Only the
    /// island goes: what stood on it is judged against the layer as cured, so it is not an
    /// island in turn; see `docs/decisions/0228`.
    #[must_use]
    pub fn removing_islands(mut self) -> Self {
        self.removing = true;
        self
    }

    /// The same fold, taking every island out and `passes` layers more of what stood on
    /// one: each pass takes what the last one left hanging, as though it were an island in
    /// turn. What is left after the last pass is named, as after the first.
    #[must_use]
    pub fn removing_hanging(mut self, passes: u32) -> Self {
        self.removing = true;
        self.passes = passes;
        self
    }

    /// Folds the next layer in, `thickness_mm` thick, and hands back what was taken out
    /// of it, which is nothing unless islands are being removed.
    pub fn push(&mut self, layer: Cured, thickness_mm: f32) -> Vec<Stretch> {
        let index = self.layers.len();
        self.layer_mm = thickness_mm;
        let below = self.below.as_ref().map(|(below, _)| below);
        self.cups.push(index, &layer, below, thickness_mm);
        let carried = match self.below.take() {
            None => self.on_plate(&layer),
            Some((below, under)) => self.carry(index, &layer, &below, &under),
        };
        // The layer below is kept as cured, islands and all, so that what stands on an
        // island is not taken for one in turn.
        let (kept, taken) = if self.removing {
            self.clear(&layer, &carried)
        } else {
            (None, Vec::new())
        };
        self.tally(index, kept.as_ref().unwrap_or(&layer), thickness_mm);
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

    /// The layer that pulls hardest on the film, in newtons, above the bottom block.
    pub fn hardest_pull(&self) -> Option<Peak> {
        self.hardest_pull
    }

    /// The layer whose area grows most over the one under it, in mm^2, above the bottom
    /// block. `None` when no layer there grows.
    pub fn largest_growth(&self) -> Option<Peak> {
        self.largest_growth
    }

    /// Islands taken out: each a piece that cured over nothing, on the one layer it did.
    pub fn removed_islands(&self) -> usize {
        self.removed_parts
    }

    /// Layers something was taken out of.
    pub fn removed_layers(&self) -> usize {
        self.removed.len()
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
        risks.extend(self.floating.iter().copied());
        risks.extend(self.levers.values().copied());
        risks.extend(self.peels.iter().chain(&self.peeling).copied());
        risks.extend(self.cups());
        risks.extend(self.thins.iter().copied());
        risks.sort_by_key(|risk| risk.layer);
        risks
    }

    fn cups(&self) -> impl Iterator<Item = Risk> + '_ {
        self.cups.found().map(|cup| Risk {
            layer: cup.peak_layer,
            at_mm: cup.at_mm,
            kind: RiskKind::SuctionCup {
                area_mm2: cup.peak_mm2,
                volume_mm3: cup.volume_mm3,
            },
        })
    }

    /// The layer the print most likely fails on: the first island or part left hanging
    /// over one taken out, since nothing cured over nothing survives, or else the neck
    /// under the most stress, the hardest peel, the deepest cup and the thinnest run.
    pub fn worst(&self) -> Option<Risk> {
        let hardest = |risks: &mut dyn Iterator<Item = Risk>| {
            risks.max_by(|a, b| severity(a).total_cmp(&severity(b)))
        };
        self.islands
            .iter()
            .chain(&self.floating)
            .min_by_key(|risk| risk.layer)
            .copied()
            .or_else(|| hardest(&mut self.levers.values().copied()))
            .or_else(|| hardest(&mut self.peels.iter().chain(&self.peeling).copied()))
            .or_else(|| hardest(&mut self.cups()))
            .or_else(|| hardest(&mut self.thins.iter().copied()))
    }

    /// Takes out of `layer` the pieces `carried` says go: its islands, and what hangs over
    /// one when that goes too. Answers what is left, when anything was taken, and the
    /// stretches taken.
    fn clear(&mut self, layer: &Cured, carried: &[Carried]) -> (Option<Cured>, Vec<Stretch>) {
        let dropped: Vec<bool> = carried.iter().map(|piece| piece.gone.is_some()).collect();
        if !dropped.contains(&true) {
            return (None, Vec::new());
        }
        self.removed_parts += carried.iter().filter(|piece| piece.island).count();
        let (kept, taken) = layer.clone().split(&dropped);
        (Some(kept), taken.stretches())
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
        raise(&mut self.hardest_pull, index, measure.pull_n);
        raise(&mut self.largest_growth, index, growth);

        let force_n = f64::from(measure.pull_n);
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
                thin: None,
                island: false,
                gone: None,
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
            thin: None,
            island: false,
            gone: None,
        };
        if touching.is_empty() {
            if piece.area_mm2 >= MIN_ISLAND_MM2 && !self.removing {
                self.islands.push(Risk {
                    layer: index,
                    at_mm: piece.centre_mm,
                    kind: RiskKind::Island {
                        area_mm2: piece.area_mm2,
                    },
                });
            }
            return Carried {
                island: true,
                gone: self.removing.then_some(0),
                ..floating
            };
        }
        let held: Vec<&Contact> = touching
            .iter()
            .filter(|contact| under[contact.below as usize].anchored)
            .collect();
        if held.is_empty() {
            let over = touching
                .iter()
                .filter_map(|contact| under[contact.below as usize].gone)
                .min();
            if let Some(depth) = over.filter(|depth| *depth < self.passes) {
                return Carried {
                    gone: Some(depth + 1),
                    ..floating
                };
            }
            // Only an island taken out leaves what stood on it to name: one kept is named
            // itself, and what stands on it goes with it.
            if over.is_some() && piece.area_mm2 >= MIN_ISLAND_MM2 {
                self.floating.push(Risk {
                    layer: index,
                    at_mm: piece.centre_mm,
                    kind: RiskKind::Floating {
                        area_mm2: piece.area_mm2,
                    },
                });
            }
            return floating;
        }

        let neck = self.narrowest(&held, under, pixel_mm2, pitch);
        if index >= self.held {
            self.weigh(index, piece, neck);
        }
        let thin = self.thin_run(index, piece, &held, under);
        Carried {
            anchored: true,
            neck,
            thin,
            island: false,
            gone: None,
        }
    }

    /// The thin run `piece` tops, carried on from the tallest one under it, and named
    /// where it starts once it is `MIN_THIN_RUN_MM` tall.
    fn thin_run(
        &mut self,
        index: usize,
        piece: &Piece,
        held: &[&Contact],
        under: &[Carried],
    ) -> Option<ThinRun> {
        let width_mm = 2.0 * (piece.area_mm2 / std::f32::consts::PI).sqrt();
        if width_mm >= THIN_MM {
            return None;
        }
        let below = held
            .iter()
            .filter_map(|contact| under[contact.below as usize].thin)
            .max_by(|a, b| a.height_mm.total_cmp(&b.height_mm));
        let mut run = below.unwrap_or(ThinRun {
            start: Risk {
                layer: index,
                at_mm: piece.centre_mm,
                kind: RiskKind::Thin { width_mm },
            },
            height_mm: 0.0,
            named: index < self.held,
        });
        run.height_mm += self.layer_mm;
        // A hair under the full height still counts: the thickness is summed in f32.
        if !run.named && run.height_mm >= MIN_THIN_RUN_MM - 1e-4 {
            run.named = true;
            let start = |risk: &Risk| (risk.layer, risk.at_mm);
            if self.thins.last().map(start) != Some(start(&run.start)) {
                self.thins.push(run.start);
            }
        }
        Some(run)
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
        let force_n = f64::from(piece.pull_n);
        let radius_mm = (neck.area_mm2 / PI).sqrt();
        let [cx, cy] = piece.centre_mm.map(f64::from);
        let offset_mm = (cx - neck.at_mm[0]).hypot(cy - neck.at_mm[1]);
        // The film lets go from one edge, so at the worst moment the pull is carried by
        // what is still stuck: half the piece, off its own centre by its gyration.
        let lever_mm = offset_mm + f64::from(piece.gyration_mm) / 2.0;
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
        RiskKind::Island { .. } | RiskKind::Floating { .. } => f32::INFINITY,
        RiskKind::Lever { stress_mpa, .. } => stress_mpa,
        RiskKind::Peel { force_n } => force_n,
        RiskKind::SuctionCup { volume_mm3, .. } => volume_mm3,
        RiskKind::Thin { width_mm } => 1.0 / width_mm.max(f32::EPSILON),
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

    /// A 20 by 20 pixel square with a 10 by 10 hole in its middle, at 0.5 mm a pixel.
    fn ring() -> Cured {
        layer(|x, y| {
            let outer = (10..30).contains(&x) && (10..30).contains(&y);
            let hole = (15..25).contains(&x) && (15..25).contains(&y);
            outer && !hole
        })
    }

    fn cups(measured: &Measured) -> Vec<Risk> {
        measured
            .risks()
            .into_iter()
            .filter(|risk| matches!(risk.kind, RiskKind::SuctionCup { .. }))
            .collect()
    }

    /// `count` rings, a millimetre of them at 20.
    fn rings(count: usize) -> Vec<Cured> {
        vec![ring(); count]
    }

    #[test]
    fn a_ring_on_the_plate_is_one_suction_cup() {
        let measured = fold(0, &rings(20));
        let found = cups(&measured);
        assert_eq!(found.len(), 1, "{found:?}");
        let RiskKind::SuctionCup {
            area_mm2,
            volume_mm3,
        } = found[0].kind
        else {
            unreachable!("filtered to cups");
        };
        // The hole is 10 px of 0.5 mm a side, 25 mm2, twenty layers of 0.05 mm.
        assert!((area_mm2 - 25.0).abs() < 1e-3, "got {area_mm2}");
        assert!((volume_mm3 - 25.0).abs() < 1e-3, "got {volume_mm3}");
    }

    #[test]
    fn a_shallow_cup_is_too_little_resin_to_name() {
        // 25 mm2 three layers deep is under 4 mm3.
        assert!(cups(&fold(0, &rings(3))).is_empty());
    }

    #[test]
    fn a_narrow_cup_is_too_little_pull_to_name() {
        let narrow = layer(|x, y| {
            let outer = (10..30).contains(&x) && (10..30).contains(&y);
            let hole = (18..22).contains(&x) && (18..22).contains(&y);
            outer && !hole
        });
        // 4 mm2 however deep: the counter of a letter.
        assert!(cups(&fold(0, &vec![narrow; 200])).is_empty());
    }

    #[test]
    fn a_ring_on_posts_lets_the_air_in_from_below() {
        let posts = layer(|x, y| {
            let corner = |at: u32| (at..at + 3).contains(&x);
            (corner(10) || corner(27)) && ((10..13).contains(&y) || (27..30).contains(&y))
        });
        let measured = fold(0, &[&[posts][..], &rings(20)].concat());
        assert!(cups(&measured).is_empty(), "air runs in between the posts");
    }

    #[test]
    fn a_ring_over_a_solid_floor_is_a_cup() {
        let measured = fold(0, &[&[square(0, 0, 40)][..], &rings(20)].concat());
        assert_eq!(
            cups(&measured).len(),
            1,
            "the floor seals it as the plate does"
        );
    }

    #[test]
    fn a_needle_is_named_once_where_it_thins() {
        // At 0.02 mm a pixel, a 3 px square is 0.06 mm across: past any support tip.
        let fine = PixelPitch { x: 0.02, y: 0.02 };
        let base = cure(
            &paint(60, 60, |x, y| u8::from(x < 40 && y < 40) * 255),
            fine,
        );
        let needle = cure(
            &paint(60, 60, |x, y| {
                u8::from((20..23).contains(&x) && (20..23).contains(&y)) * 255
            }),
            fine,
        );
        let thins = |needle_layers: usize| {
            let stack = [vec![base.clone()], vec![needle.clone(); needle_layers]].concat();
            fold(0, &stack)
                .risks()
                .into_iter()
                .filter(|risk| matches!(risk.kind, RiskKind::Thin { .. }))
                .collect::<Vec<Risk>>()
        };
        // A millimetre is twenty layers of 0.05 mm.
        let thin = thins(30);
        assert_eq!(thin.len(), 1, "{thin:?}");
        assert_eq!(thin[0].layer, 1, "named where it starts");
        assert!(
            thins(5).is_empty(),
            "a quarter of a millimetre is the tip of something, not a wire"
        );
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
        // The slab's centre is 9.75 mm off the pin on both axes, and half its gyration —
        // a 20 mm square gyrates 20 / sqrt(6) about its centre — is added to that.
        let offset = 9.75 * std::f32::consts::SQRT_2;
        let gyration = 20.0 / 6.0_f32.sqrt();
        assert!(
            (lever_mm - offset - gyration / 2.0).abs() < 0.05,
            "the slab's centre is far off the pin: {lever_mm}"
        );
    }

    #[test]
    fn removing_islands_takes_the_island_alone_and_names_nothing() {
        let base = square(0, 0, 10);
        let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
        let mut measured = Measured::new(0).removing_islands();
        let taken: Vec<usize> = [base, both.clone(), both]
            .into_iter()
            .map(|layer| measured.push(layer, 0.05).len())
            .collect();
        // The second part is 30 rows tall; only its first layer cured over nothing.
        assert_eq!(taken, vec![0, 30, 0], "what stood on the island stays");
        assert_eq!(measured.removed_islands(), 1);
        assert_eq!(measured.removed_layers(), 1);
        let risks = measured.risks();
        assert_eq!(risks.len(), 1, "{risks:?}");
        assert!(
            matches!(risks[0].kind, RiskKind::Floating { .. }) && risks[0].layer == 2,
            "what stood on it is named once, as hanging over nothing"
        );
        assert!(measured.removed_from(2).is_empty());
        // The base on three layers, 5 x 5 mm, and the part's second layer, 15 x 15 mm.
        let expected_mm3 = 25.0 * 0.15 + 225.0 * 0.05;
        assert!((measured.volume_mm3() - expected_mm3).abs() < 1e-3);
    }

    #[test]
    fn a_run_of_layers_pulling_past_the_limit_is_named_once_at_its_hardest() {
        // A 30 mm square pulls about 4 N; a pitch of 3 mm makes the same pixels 180 mm.
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
    fn a_plate_centred_on_a_stem_as_thin_as_a_wire_is_still_a_lever() {
        // The coupon of the manual run: a 30 x 30 mm plate on a 1.5 mm stem. Its centre
        // sits over the stem, so only the peel's own arm finds it (P-08).
        let pitch = PixelPitch { x: 0.05, y: 0.05 };
        let round = |cx: f64, cy: f64, radius: f64| {
            cure(
                &paint(700, 700, |x, y| {
                    let (dx, dy) = (f64::from(x) + 0.5 - cx, f64::from(y) + 0.5 - cy);
                    u8::from(dx * dx + dy * dy <= radius * radius) * 255
                }),
                pitch,
            )
        };
        let square = |side: u32| {
            cure(
                &paint(700, 700, |x, y| {
                    let inside = (350 - side / 2..350 + side / 2).contains(&x)
                        && (350 - side / 2..350 + side / 2).contains(&y);
                    u8::from(inside) * 255
                }),
                pitch,
            )
        };
        let mut measured = Measured::new(0);
        for _ in 0..3 {
            measured.push(round(350.0, 350.0, 15.0), 0.05);
        }
        measured.push(square(600), 0.05);

        let risks = measured.risks();
        assert_eq!(risks.len(), 1, "{risks:?}");
        let RiskKind::Lever {
            stress_mpa,
            neck_mm2,
            ..
        } = risks[0].kind
        else {
            panic!("a lever, not {:?}", risks[0].kind);
        };
        assert!(
            (neck_mm2 - 1.77).abs() < 0.05,
            "the stem is the neck: {neck_mm2}"
        );
        assert!(
            stress_mpa > NECK_LIMIT_MPA as f32,
            "past the limit: {stress_mpa} MPa"
        );
    }

    #[test]
    fn each_pass_takes_one_layer_more_of_what_hangs() {
        let base = square(0, 0, 10);
        let both = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
        let stack = [base, both.clone(), both.clone(), both.clone(), both];
        let fold = |passes: u32| {
            let mut measured = Measured::new(0).removing_hanging(passes);
            let taken: Vec<usize> = stack
                .iter()
                .map(|layer| measured.push(layer.clone(), 0.05).len())
                .collect();
            (taken, measured)
        };

        let (taken, measured) = fold(1);
        assert_eq!(
            taken,
            vec![0, 30, 30, 0, 0],
            "the island and one layer over it"
        );
        assert_eq!(
            measured.removed_islands(),
            1,
            "one island, however many passes"
        );
        let risks = measured.risks();
        assert_eq!(risks.len(), 1, "{risks:?}");
        assert_eq!(
            risks[0].layer, 3,
            "what is left hanging is named where it starts"
        );

        let (taken, measured) = fold(3);
        assert_eq!(taken, vec![0, 30, 30, 30, 30], "three passes reach the top");
        assert!(
            measured.risks().is_empty(),
            "nothing is left hanging to name"
        );
    }

    #[test]
    fn a_floating_part_loses_its_first_layer_alone_however_many_it_has() {
        let base = square(0, 0, 10);
        let floating = layer(|x, y| (x < 10 && y < 10) || (x >= 30 && y >= 30));
        let mut measured = Measured::new(0).removing_islands();
        // The base alone, then four layers with a second part beside it, then the base
        // alone again, then the part back: two parts, over five layers.
        for layer in [
            base.clone(),
            floating.clone(),
            floating.clone(),
            floating.clone(),
            floating.clone(),
            base,
            floating,
        ] {
            measured.push(layer, 0.05);
        }
        assert_eq!(measured.removed_islands(), 2);
        assert_eq!(measured.removed_layers(), 2, "the first layer of each part");
    }

    #[test]
    fn the_same_slab_centred_on_a_broad_neck_is_not() {
        let post = square(15, 15, 30);
        let slab = square(5, 5, 50);
        assert!(fold(0, &[post.clone(), post, slab]).risks().is_empty());
    }
}
