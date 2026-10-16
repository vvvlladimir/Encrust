//! The islands a print cures at its own pixel and layer, found by drawing only a window over
//! each low point of the surface. See `docs/design/orientation.md`.

mod births;

use core_analysis::{Cured, cure, islands};
use core_geometry::{FastMap, Mesh, Scalar, Vec2, Vec3};
use core_pipeline::Tolerance;
use core_raster::{RasterSettings, Rasterizer, ScanlineRasterizer};
use core_slicer::{Contour, Layer, PlaneSliceEngine, SliceEngine, Source};
use rayon::prelude::*;

use crate::error::EngineError;
use crate::run::Run;
use crate::trial::islands::births::births;

/// How far a window reaches round a low point, millimetres: twice the widest island a print
/// was seen to cure beside the point it grew from.
const REACH_MM: Scalar = 1.5;
/// How far over a low point its island may first cure, millimetres: a print was seen to
/// light one at most 0.15 mm up, three layers of 0.05 mm; this leaves two more.
const RISE_MM: Scalar = 0.25;
/// Low points closer than this share a window, millimetres: an island stays inside a
/// window opened half a millimetre off its own.
const SHARED_MM: Scalar = 0.5;
/// The cells the plate's faces are filed in, millimetres across and high: low, since a
/// window reads a band a few layers high out of a dense mesh.
const CELL_MM: Vec3 = Vec3::new(1.0, 1.0, 0.25);
/// Dark pixels round what a window draws, so that a blur never reaches its edge.
const PAD_PX: i64 = 4;
/// Fractions of a pixel a centre is kept to, so that one island drawn in two windows is
/// counted once.
const SUBPIXEL: f32 = 64.0;

/// An island a window found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Born {
    layer: usize,
    /// Its centre on the whole panel, in `SUBPIXEL`ths of a pixel.
    centre: [i64; 2],
    /// The corner of the window that found it, panel pixels.
    corner_px: [i64; 2],
}

impl Born {
    /// Whether `other` is the same piece of the same layer, its centre within a pixel.
    fn is(&self, other: &Born) -> bool {
        self.layer == other.layer
            && (0..2).all(|axis| (self.centre[axis] - other.centre[axis]).abs() < SUBPIXEL as i64)
    }
}

/// The islands `run` would cure at full resolution, born at the low points of `model`, which
/// `run` cuts where it stands.
pub(crate) fn islands_born(run: &Run, model: &Mesh) -> Result<usize, EngineError> {
    let heights: Vec<Scalar> = (0..run.layer_count())
        .filter_map(|layer| run.windows().height_of(layer))
        .collect();
    let probe = Probe {
        panel: run.panel(),
        tolerance: Tolerance::of(&run.job().material),
        heights: &heights,
        rise: layer_rise(&heights),
    };
    let lows = shared(births(model, REACH_MM), &heights);
    let boxes: Vec<(Vec3, Vec3)> = lows
        .iter()
        .filter_map(|low| probe.window_box(*low))
        .collect();
    let filed = Filed::of(&run.solid().mesh, &boxes);
    let found: Vec<Vec<Born>> = lows
        .par_iter()
        .map(|low| probe.near(&run.solid().mesh, &filed, *low))
        .collect::<Result<_, _>>()?;
    let mut found: Vec<Born> = found.into_iter().flatten().collect();
    found.retain(|born| probe.on_panel(*born));
    found.sort_unstable();
    // One island drawn in two windows rounds its centre apart by float noise.
    found.dedup_by(|later, kept| kept.is(later));
    probe.confirmed(run.solid(), &found)
}

/// How many layers over a low point its island may first cure.
fn layer_rise(heights: &[Scalar]) -> usize {
    let step = match heights {
        [first, second, ..] => second - first,
        _ => return 0,
    };
    (RISE_MM / step.max(Scalar::EPSILON)).ceil() as usize
}

/// `lows` with one kept of those that share a window and first cure on one layer.
fn shared(lows: Vec<Vec3>, heights: &[Scalar]) -> Vec<Vec3> {
    let mut kept: FastMap<(i32, i32, usize), Vec3> = FastMap::default();
    for low in lows {
        let cell = |v: Scalar| (v / SHARED_MM).floor() as i32;
        let layer = heights.partition_point(|z| *z <= low.z);
        kept.entry((cell(low.x), cell(low.y), layer)).or_insert(low);
    }
    kept.into_values().collect()
}

/// The faces of a mesh filed by the cells their boxes reach, only where a window looks.
struct Filed {
    cells: FastMap<[i32; 3], Vec<u32>>,
}

fn cell_of(at: Vec3) -> [i32; 3] {
    (at / CELL_MM).floor().as_ivec3().to_array()
}

impl Filed {
    /// The faces of `mesh` in the cells the windows `boxes` look into.
    fn of(mesh: &Mesh, boxes: &[(Vec3, Vec3)]) -> Self {
        let Some(looked) = Looked::of(boxes) else {
            return Self {
                cells: FastMap::default(),
            };
        };
        let found: Vec<([i32; 3], u32)> = mesh
            .faces
            .par_iter()
            .enumerate()
            .flat_map_iter(|(at, face)| {
                let reach = face_box(mesh, *face).filter(|(lo, hi)| looked.may_reach(*lo, *hi));
                reach
                    .into_iter()
                    .flat_map(|(lo, hi)| cells_between(lo, hi))
                    .filter(|key| looked.looks_at(*key))
                    .map(move |key| (key, at as u32))
            })
            .collect();
        let mut cells: FastMap<[i32; 3], Vec<u32>> = FastMap::default();
        for (key, at) in found {
            cells.entry(key).or_default().push(at);
        }
        Self { cells }
    }

    /// Every face of `mesh` whose box may reach the box `lo`..`hi`, each once: from the
    /// first cell of the box it reaches.
    fn faces(&self, mesh: &Mesh, lo: Vec3, hi: Vec3) -> Vec<u32> {
        let first = cell_of(lo);
        let mut found = Vec::new();
        for key in cells_between(lo, hi) {
            for &at in self.cells.get(&key).into_iter().flatten() {
                let owner = mesh
                    .faces
                    .get(at as usize)
                    .and_then(|face| face_box(mesh, *face))
                    .map(|(low, _)| {
                        let own = cell_of(low);
                        [0, 1, 2].map(|axis| own[axis].max(first[axis]))
                    });
                if owner == Some(key) {
                    found.push(at);
                }
            }
        }
        found
    }
}

/// The cells any window looks into, one bit a cell over the box they span, so that a face
/// is turned away without a lookup.
struct Looked {
    first: [i32; 3],
    last: [i32; 3],
    size: [usize; 3],
    bits: Vec<u64>,
}

impl Looked {
    fn of(boxes: &[(Vec3, Vec3)]) -> Option<Self> {
        let lo = boxes.iter().map(|b| b.0).reduce(Vec3::min)?;
        let hi = boxes.iter().map(|b| b.1).reduce(Vec3::max)?;
        let (first, last) = (cell_of(lo), cell_of(hi));
        let size = [0, 1, 2].map(|axis| (last[axis] - first[axis] + 1) as usize);
        let mut looked = Self {
            first,
            last,
            size,
            bits: vec![0; (size[0] * size[1] * size[2]).div_ceil(64)],
        };
        for (lo, hi) in boxes {
            for key in cells_between(*lo, *hi) {
                if let Some(bit) = looked.bit(key) {
                    looked.bits[bit / 64] |= 1 << (bit % 64);
                }
            }
        }
        Some(looked)
    }

    fn bit(&self, key: [i32; 3]) -> Option<usize> {
        let at = [0, 1, 2].map(|axis| key[axis] - self.first[axis]);
        if (0..3).any(|axis| at[axis] < 0 || at[axis] as usize >= self.size[axis]) {
            return None;
        }
        let [i, j, k] = at.map(|at| at as usize);
        Some((k * self.size[1] + j) * self.size[0] + i)
    }

    fn looks_at(&self, key: [i32; 3]) -> bool {
        self.bit(key)
            .is_some_and(|bit| self.bits[bit / 64] & (1 << (bit % 64)) != 0)
    }

    /// Whether the box `lo`..`hi` overlaps the span the windows look into at all.
    fn may_reach(&self, lo: Vec3, hi: Vec3) -> bool {
        let (lo, hi) = (cell_of(lo), cell_of(hi));
        (0..3).all(|axis| hi[axis] >= self.first[axis] && lo[axis] <= self.last[axis])
    }
}

fn cells_between(lo: Vec3, hi: Vec3) -> impl Iterator<Item = [i32; 3]> {
    let (lo, hi) = (cell_of(lo), cell_of(hi));
    (lo[0]..=hi[0]).flat_map(move |i| {
        (lo[1]..=hi[1]).flat_map(move |j| (lo[2]..=hi[2]).map(move |k| [i, j, k]))
    })
}

fn face_box(mesh: &Mesh, face: [u32; 3]) -> Option<(Vec3, Vec3)> {
    let [a, b, c] = face.map(|index| mesh.vertices.get(index as usize).copied());
    let (a, b, c) = (a?, b?, c?);
    Some((a.min(b).min(c), a.max(b).max(c)))
}

/// What every window is cut and drawn with: the print's own panel, resin and layers.
struct Probe<'a> {
    panel: &'a RasterSettings,
    tolerance: Tolerance,
    heights: &'a [Scalar],
    rise: usize,
}

/// A window of the panel: the pixel its corner is, and the panel it draws as.
struct Window {
    corner_px: [i64; 2],
    settings: RasterSettings,
}

impl Probe<'_> {
    /// The islands a window over `low` draws out of the contours that never leave it: every
    /// island a print would cure there, and some that a wall reaching in from outside holds.
    fn near(&self, mesh: &Mesh, filed: &Filed, low: Vec3) -> Result<Vec<Born>, EngineError> {
        let Some((bottom, planes)) = self.planes(low) else {
            return Ok(Vec::new());
        };
        let Some((lo, hi)) = self.window_box(low) else {
            return Ok(Vec::new());
        };
        let Some(cut) = cut_out(mesh, &filed.faces(mesh, lo, hi), lo, hi) else {
            return Ok(Vec::new());
        };
        let sliced = PlaneSliceEngine.slice_at(&cut, planes)?;
        let window = self.window(Vec2::new(low.x, low.y));
        let within = |contour: &&Contour| {
            contour
                .points
                .iter()
                .all(|p| (p.x - low.x).abs() < REACH_MM && (p.y - low.y).abs() < REACH_MM)
        };
        let cured: Vec<Cured> = sliced
            .layers
            .iter()
            .enumerate()
            .map(|(at, layer)| {
                let whole: Vec<Contour> = layer.contours.iter().filter(within).cloned().collect();
                self.cured(&whole, &window, bottom + at)
            })
            .collect::<Result<_, _>>()?;
        Ok(self.found(&cured, &window, bottom))
    }

    /// The print's planes a window over `low` cuts, from the one under it, and the index of
    /// that first plane.
    fn planes(&self, low: Vec3) -> Option<(usize, &[Scalar])> {
        let first = self.heights.partition_point(|z| *z <= low.z);
        let last = (first + self.rise).min(self.heights.len().saturating_sub(1));
        let bottom = first.checked_sub(1)?;
        let planes = self.heights.get(bottom..=last)?;
        (planes.len() >= 2).then_some((bottom, planes))
    }

    /// The box a window over `low` cuts out, up its planes.
    fn window_box(&self, low: Vec3) -> Option<(Vec3, Vec3)> {
        let (_, planes) = self.planes(low)?;
        Some((
            Vec3::new(low.x - REACH_MM, low.y - REACH_MM, *planes.first()?),
            Vec3::new(low.x + REACH_MM, low.y + REACH_MM, *planes.last()?),
        ))
    }

    /// The islands of `cured` against the layer under each, numbered from `bottom`.
    fn found(&self, cured: &[Cured], window: &Window, bottom: usize) -> Vec<Born> {
        let mut found = Vec::new();
        for (at, pair) in cured.windows(2).enumerate() {
            let (below, above) = (&pair[0], &pair[1]);
            for (piece, island) in above.pieces().iter().zip(islands(above, below)) {
                if island {
                    found.push(Born {
                        layer: bottom + at + 1,
                        centre: self.centre(window, piece.centre_mm),
                        corner_px: window.corner_px,
                    });
                }
            }
        }
        found
    }

    /// A piece's centre on the whole panel, in `SUBPIXEL`ths of a pixel.
    fn centre(&self, window: &Window, centre_mm: [f32; 2]) -> [i64; 2] {
        let pitch = [self.panel.pitch.x, self.panel.pitch.y];
        [0, 1].map(|axis| {
            let px = centre_mm[axis] / pitch[axis] + window.corner_px[axis] as f32;
            (px * SUBPIXEL).round() as i64
        })
    }

    /// Whether `born` lies on the panel: a pose that overflows it cures nothing past its edge.
    fn on_panel(&self, born: Born) -> bool {
        let inside =
            |at: i64, size_px: u32| (0..i64::from(size_px) * SUBPIXEL as i64).contains(&at);
        inside(born.centre[0], self.panel.width_px) && inside(born.centre[1], self.panel.height_px)
    }

    /// The window `REACH_MM` round `centre_mm`, on the panel's own pixel grid.
    fn window(&self, centre_mm: Vec2) -> Window {
        let pitch = [self.panel.pitch.x, self.panel.pitch.y];
        let centre = [centre_mm.x, centre_mm.y];
        let reach_px = self.reach_px();
        self.window_at(
            [0, 1].map(|axis| (centre[axis] / pitch[axis]).floor() as i64 - reach_px[axis]),
        )
    }

    /// How many pixels a window reaches each way from its middle, padding included.
    fn reach_px(&self) -> [i64; 2] {
        [self.panel.pitch.x, self.panel.pitch.y]
            .map(|pitch| (REACH_MM / pitch).ceil() as i64 + PAD_PX)
    }

    /// The window whose corner is the pixel `corner_px`.
    fn window_at(&self, corner_px: [i64; 2]) -> Window {
        let reach_px = self.reach_px();
        Window {
            corner_px,
            settings: RasterSettings {
                width_px: (2 * reach_px[0] + 1) as u32,
                height_px: (2 * reach_px[1] + 1) as u32,
                mirror_x: false,
                mirror_y: false,
                ..*self.panel
            },
        }
    }

    /// `contours` of layer `index`, moved by the resin's offsets, drawn into `window`.
    fn cured(
        &self,
        contours: &[Contour],
        window: &Window,
        index: usize,
    ) -> Result<Cured, EngineError> {
        let pitch = window.settings.pitch;
        let corner = Vec2::new(
            window.corner_px[0] as f32 * pitch.x,
            window.corner_px[1] as f32 * pitch.y,
        );
        let size = Vec2::new(
            window.settings.width_px as f32 * pitch.x,
            window.settings.height_px as f32 * pitch.y,
        );
        let edge = Vec2::new(pitch.x, pitch.y) * PAD_PX as f32;
        let moved: Vec<Contour> = contours
            .iter()
            .filter_map(|contour| {
                let ring: Vec<Vec2> = contour.points.iter().map(|p| *p - corner).collect();
                let ring = clipped(&ring, -edge, size + edge);
                (ring.len() >= 3).then(|| Contour::new(ring, contour.winding))
            })
            .collect();
        let layer = Layer::new(0.0, moved);
        let offset = self.tolerance.applied(&layer, index as u32);
        let rastered = ScanlineRasterizer
            .rasterize(&offset, &window.settings)
            .map_err(EngineError::Panel)?;
        Ok(cure(&rastered.runs, pitch))
    }

    /// Those of `found` a cut across the whole plate confirms: the layer under each holds
    /// nothing up where it cures.
    fn confirmed(&self, source: &impl Source, found: &[Born]) -> Result<usize, EngineError> {
        let mut layers: Vec<usize> = found
            .iter()
            .flat_map(|born| [born.layer - 1, born.layer])
            .collect();
        layers.sort_unstable();
        layers.dedup();
        let planes: Vec<Scalar> = layers
            .iter()
            .filter_map(|at| self.heights.get(*at).copied())
            .collect();
        if found.is_empty() || planes.len() != layers.len() {
            return Ok(0);
        }
        let sliced = source.slice_at(&planes)?;
        let layer_of = |at: usize| {
            layers
                .binary_search(&at)
                .ok()
                .and_then(|at| sliced.layers.get(at))
        };
        let kept = found
            .par_iter()
            .map(|born| {
                let (Some(below), Some(above)) = (layer_of(born.layer - 1), layer_of(born.layer))
                else {
                    return Ok(false);
                };
                self.still_island(*born, below, above)
            })
            .collect::<Result<Vec<bool>, EngineError>>()?;
        Ok(kept.into_iter().filter(|kept| *kept).count())
    }

    /// Whether the piece `born` names is an island on the whole layers `below` and `above`,
    /// drawn into the window that found it.
    fn still_island(&self, born: Born, below: &Layer, above: &Layer) -> Result<bool, EngineError> {
        let window = self.window_at(born.corner_px);
        let under = self.cured(&below.contours, &window, born.layer - 1)?;
        let over = self.cured(&above.contours, &window, born.layer)?;
        let flags = islands(&over, &under);
        Ok(over.pieces().iter().zip(flags).any(|(piece, island)| {
            island
                && born.is(&Born {
                    centre: self.centre(&window, piece.centre_mm),
                    ..born
                })
        }))
    }
}

/// The faces of `mesh` named in `faces` whose boxes reach `lo`..`hi`, as a mesh of their own.
fn cut_out(mesh: &Mesh, faces: &[u32], lo: Vec3, hi: Vec3) -> Option<Mesh> {
    let mut renumbered: FastMap<u32, u32> = FastMap::default();
    let mut vertices = Vec::new();
    let mut kept = Vec::new();
    for &at in faces {
        let face = *mesh.faces.get(at as usize)?;
        let (low, high) = face_box(mesh, face)?;
        if high.cmplt(lo).any() || low.cmpgt(hi).any() {
            continue;
        }
        kept.push(face.map(|index| {
            *renumbered.entry(index).or_insert_with(|| {
                vertices.push(mesh.vertices[index as usize]);
                (vertices.len() - 1) as u32
            })
        }));
    }
    (!kept.is_empty()).then(|| Mesh::new(vertices, kept))
}

/// `ring` cut to the box `lo`..`hi`: the same region inside it, closed along its edge.
fn clipped(ring: &[Vec2], lo: Vec2, hi: Vec2) -> Vec<Vec2> {
    let mut out = ring.to_vec();
    for (axis, bound, keep_above) in [
        (0, lo.x, true),
        (0, hi.x, false),
        (1, lo.y, true),
        (1, hi.y, false),
    ] {
        let input = std::mem::take(&mut out);
        let inside = |p: Vec2| {
            if keep_above {
                p[axis] >= bound
            } else {
                p[axis] <= bound
            }
        };
        for (at, &a) in input.iter().enumerate() {
            let b = input[(at + 1) % input.len()];
            if inside(a) {
                out.push(a);
            }
            if inside(a) != inside(b) {
                out.push(a + (b - a) * ((bound - a[axis]) / (b[axis] - a[axis])));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use core_analysis::RiskKind;
    use core_format::ExposurePlan;
    use core_geometry::Transform;
    use core_pipeline::{PanelOverrides, SlicedFormat};
    use printer_profiles::{MaterialProfile, PrinterProfile};

    use super::*;
    use crate::cut::Cutting;
    use crate::plate::{Model, Plate};

    /// Panel of 600 x 400 pixels at a 0.1 mm pitch: 60 x 40 mm of build area.
    const PROFILE: &str = r#"
name = "Test panel"
manufacturer = "Test"

[display]
width_px = 600
height_px = 400
width_mm = 60.0
height_mm = 40.0

[build_volume]
x = 60.0
y = 40.0
z = 80.0
"#;

    /// Boxes laid into one mesh, each twelve triangles wound outward.
    fn boxes(parts: &[(Vec3, Vec3)]) -> Mesh {
        let mut mesh = Mesh::default();
        for (mins, maxs) in parts {
            let offset = mesh.vertices.len() as u32;
            mesh.vertices.extend((0..8).map(|corner| {
                let pick = |axis: usize| {
                    if corner >> axis & 1 == 0 {
                        mins[axis]
                    } else {
                        maxs[axis]
                    }
                };
                Vec3::new(pick(0), pick(1), pick(2))
            }));
            let faces = [
                [0, 2, 1],
                [1, 2, 3],
                [4, 5, 6],
                [5, 7, 6],
                [0, 1, 4],
                [1, 5, 4],
                [2, 6, 3],
                [3, 6, 7],
                [0, 4, 2],
                [2, 4, 6],
                [1, 3, 5],
                [3, 7, 5],
            ];
            mesh.faces
                .extend(faces.iter().map(|face| face.map(|index| index + offset)));
        }
        mesh
    }

    fn run_of(mesh: &Mesh) -> Run {
        let printer = PrinterProfile::from_toml_str(PROFILE, Path::new("inline.toml"))
            .expect("the inline profile parses");
        let material = MaterialProfile::default();
        let plate = Plate {
            models: vec![Model::placed(Arc::new(mesh.clone()), Transform::default())],
            cutting: Cutting::uniform(material.layer_height_mm),
            printer,
            material,
            panel: PanelOverrides::default(),
            exposure: ExposurePlan::default(),
            remove_islands: false,
            hanging_passes: 0,
            format: SlicedFormat::Goo,
            raster_window: 4,
            created_unix_s: 0,
        };
        Run::measuring(&plate).expect("the plate has layers")
    }

    /// What a full cure of `run` counts as islands, to hold the windows to.
    fn cured_islands(run: &Run) -> usize {
        let measured = run.measure(&mut ()).expect("cures").expect("not cancelled");
        measured
            .risks()
            .iter()
            .filter(|risk| matches!(risk.kind, RiskKind::Island { .. }))
            .count()
    }

    /// A 10 mm block on the plate, and beside it what `rest` adds.
    fn block_and(rest: (Vec3, Vec3)) -> Mesh {
        boxes(&[
            (Vec3::new(20.0, 15.0, 0.0), Vec3::new(30.0, 25.0, 1.0)),
            rest,
        ])
    }

    #[test]
    fn a_cube_floating_over_the_plate_is_one_island() {
        let mesh = block_and((Vec3::new(40.0, 20.0, 2.0), Vec3::new(41.0, 21.0, 3.0)));
        let run = run_of(&mesh);
        let found = islands_born(&run, &mesh).expect("counts");
        assert_eq!(
            found, 1,
            "the floating cube's first layer stands on nothing"
        );
        assert_eq!(found, cured_islands(&run), "a full cure counts the same");
    }

    #[test]
    fn a_cube_standing_on_the_block_is_no_island() {
        let mesh = block_and((Vec3::new(24.5, 19.5, 1.0), Vec3::new(25.5, 20.5, 2.0)));
        let run = run_of(&mesh);
        assert_eq!(islands_born(&run, &mesh).expect("counts"), 0);
        assert_eq!(cured_islands(&run), 0, "a full cure agrees");
    }

    #[test]
    fn a_cube_floating_over_the_block_counts_as_a_full_cure_does() {
        // Over the block's top with a millimetre of air under it.
        let mesh = block_and((Vec3::new(24.5, 19.5, 2.0), Vec3::new(25.5, 20.5, 3.0)));
        let run = run_of(&mesh);
        let found = islands_born(&run, &mesh).expect("counts");
        assert_eq!(
            found, 1,
            "the cube's first layer stands on air over the block"
        );
        assert_eq!(found, cured_islands(&run), "a full cure counts the same");
    }
}
