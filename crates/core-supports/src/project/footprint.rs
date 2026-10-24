use core_geometry::glam::IVec2;
use core_geometry::{FastMap, FastSet, Region, Scalar, Vec2, Vec3};

use super::{centre_of, lowest_surface};
use crate::placed::Placed;

/// How finely the footprint is read, as cells across the finest spacing asked for.
const CELLS_PER_SPACING: Scalar = 4.0;

/// Bounds on a footprint cell, millimetres: finer costs memory with the patch's area,
/// coarser rounds an offset off by more than a tip is wide.
const MIN_CELL_MM: Scalar = 0.1;
const MAX_CELL_MM: Scalar = 0.5;

/// A walk along a ring steps to the nearest unvisited cell this many cells away at most,
/// which bridges the corners an 8-connected band of one cell can leave.
const WALK_REACH: i32 = 2;

/// A painted patch seen from the plate: the lowest surface over each cell and how far the cell
/// lies inside the outline, for rim offsets and rings. See `docs/design/supports.md`.
pub(super) struct Footprint {
    cell_mm: Scalar,
    /// The cell at index 0, one cell outside the patch on every side.
    origin: IVec2,
    width: i32,
    height: i32,
    /// The lowest surface over each cell, or NaN where the patch is not.
    z: Vec<Scalar>,
    /// Millimetres from each cell's middle in to the patch's outline; nought outside.
    depth_mm: Vec<Scalar>,
}

impl Footprint {
    /// The footprint of `region` read at a quarter of `spacing_mm`, or `None` when it covers
    /// nothing seen from above.
    pub(super) fn of(placed: &Placed, region: &Region, spacing_mm: Scalar) -> Option<Self> {
        let cell_mm = (spacing_mm / CELLS_PER_SPACING).clamp(MIN_CELL_MM, MAX_CELL_MM);
        let lowest = lowest_surface(placed, region, cell_mm);
        let mut keys = lowest.keys();
        let first = IVec2::from(*keys.next()?);
        let (min, max) = keys.fold((first, first), |(min, max), cell| {
            (min.min(IVec2::from(*cell)), max.max(IVec2::from(*cell)))
        });
        let origin = min - IVec2::ONE;
        let size = max - min + IVec2::splat(3);
        let mut z = vec![Scalar::NAN; (size.x * size.y) as usize];
        for (cell, height) in &lowest {
            let at = IVec2::from(*cell) - origin;
            z[(at.y * size.x + at.x) as usize] = *height;
        }
        let depth_mm = depths(&z, size.x, size.y, cell_mm);
        Some(Self {
            cell_mm,
            origin,
            width: size.x,
            height: size.y,
            z,
            depth_mm,
        })
    }

    pub(super) fn cell_mm(&self) -> Scalar {
        self.cell_mm
    }

    /// How far inside the outline the plate point `at` lies, millimetres; nought outside.
    pub(super) fn depth_at(&self, at: Vec2) -> Scalar {
        let cell = (at / self.cell_mm).floor().as_ivec2();
        self.index(cell).map_or(0.0, |index| self.depth_mm[index])
    }

    /// Points `spacing_mm` apart along the ring `depth_mm` inside the outline, on the
    /// surface. Each separate loop of the ring is walked on its own.
    pub(super) fn ring(&self, depth_mm: Scalar, spacing_mm: Scalar) -> Vec<Vec3> {
        let depth_mm = depth_mm.max(self.cell_mm / 2.0);
        let half = self.cell_mm / 2.0;
        let mut band: Vec<IVec2> = self
            .cells()
            .filter(|(_, depth)| *depth >= depth_mm - half && *depth < depth_mm + half)
            .map(|(cell, _)| cell)
            .collect();
        band.sort_unstable_by_key(|cell| (cell.y, cell.x));
        let mut left: FastSet<IVec2> = band.iter().copied().collect();

        let mut points = Vec::new();
        for start in band {
            if !left.remove(&start) {
                continue;
            }
            let mut last = self.point(start);
            points.extend(last);
            let mut at = start;
            while let Some(next) = nearest_left(&left, at) {
                left.remove(&next);
                at = next;
                if let Some(point) = self.point(next)
                    && last
                        .is_none_or(|last| last.truncate().distance(point.truncate()) >= spacing_mm)
                {
                    points.push(point);
                    last = Some(point);
                }
            }
        }
        points
    }

    /// Points no closer than `spacing_mm` scattered over cells at least `depth_mm` inside the
    /// outline, on the surface, in a per-cell hash order so the same patch scatters the same way.
    pub(super) fn scattered(&self, depth_mm: Scalar, spacing_mm: Scalar) -> Vec<Vec3> {
        let mut cells: Vec<IVec2> = self
            .cells()
            .filter(|(_, depth)| *depth >= depth_mm.max(self.cell_mm / 2.0))
            .map(|(cell, _)| cell)
            .collect();
        cells.sort_unstable_by_key(|cell| scramble(*cell));
        let mut taken: FastMap<[i32; 2], Vec<Vec2>> = FastMap::default();
        let mut points = Vec::new();
        for cell in cells {
            let Some(point) = self.point(cell) else {
                continue;
            };
            let flat = point.truncate();
            let bucket = (flat / spacing_mm).floor().as_ivec2();
            let crowded = (-1..=1).any(|x| {
                (-1..=1).any(|y| {
                    taken
                        .get(&(bucket + IVec2::new(x, y)).to_array())
                        .is_some_and(|near| {
                            near.iter().any(|other| other.distance(flat) < spacing_mm)
                        })
                })
            });
            if !crowded {
                taken.entry(bucket.to_array()).or_default().push(flat);
                points.push(point);
            }
        }
        points
    }

    /// Every cell under the patch, in plate cells, with its depth.
    fn cells(&self) -> impl Iterator<Item = (IVec2, Scalar)> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width).filter_map(move |x| {
                let index = (y * self.width + x) as usize;
                (!self.z[index].is_nan())
                    .then(|| (self.origin + IVec2::new(x, y), self.depth_mm[index]))
            })
        })
    }

    /// The surface over the middle of a plate cell, if the patch is there.
    fn point(&self, cell: IVec2) -> Option<Vec3> {
        let z = self.z[self.index(cell)?];
        (!z.is_nan()).then(|| centre_of(cell.to_array(), self.cell_mm).extend(z))
    }

    fn index(&self, cell: IVec2) -> Option<usize> {
        let at = cell - self.origin;
        (at.x >= 0 && at.y >= 0 && at.x < self.width && at.y < self.height)
            .then(|| (at.y * self.width + at.x) as usize)
    }
}

/// The unvisited cell nearest `at` within `WALK_REACH`, the lowest row and column first
/// among equals.
fn nearest_left(left: &FastSet<IVec2>, at: IVec2) -> Option<IVec2> {
    let mut best: Option<(i32, IVec2)> = None;
    for y in -WALK_REACH..=WALK_REACH {
        for x in -WALK_REACH..=WALK_REACH {
            let cell = at + IVec2::new(x, y);
            let far = x * x + y * y;
            if left.contains(&cell) && best.is_none_or(|(near, _)| far < near) {
                best = Some((far, cell));
            }
        }
    }
    best.map(|(_, cell)| cell)
}

/// Millimetres from each cell middle to the outline of the non-NaN cells, by the two-pass 8SSEDT
/// sweep; accurate to within a cell, outside cells are nought.
fn depths(z: &[Scalar], width: i32, height: i32, cell_mm: Scalar) -> Vec<Scalar> {
    const FAR: IVec2 = IVec2::splat(1 << 14);
    let mut near: Vec<IVec2> = z
        .iter()
        .map(|z| if z.is_nan() { IVec2::ZERO } else { FAR })
        .collect();
    let offer = |near: &mut [IVec2], x: i32, y: i32, step: IVec2| {
        let (from_x, from_y) = (x + step.x, y + step.y);
        if from_x < 0 || from_y < 0 || from_x >= width || from_y >= height {
            return;
        }
        let through = near[(from_y * width + from_x) as usize] + step;
        let here = &mut near[(y * width + x) as usize];
        if through.length_squared() < here.length_squared() {
            *here = through;
        }
    };
    for y in 0..height {
        for x in 0..width {
            for step in [(-1, 0), (0, -1), (-1, -1), (1, -1)] {
                offer(&mut near, x, y, IVec2::from(step));
            }
        }
        for x in (0..width).rev() {
            offer(&mut near, x, y, IVec2::new(1, 0));
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            for step in [(1, 0), (0, 1), (1, 1), (-1, 1)] {
                offer(&mut near, x, y, IVec2::from(step));
            }
        }
        for x in 0..width {
            offer(&mut near, x, y, IVec2::new(-1, 0));
        }
    }
    near.iter()
        .map(|to| (to.as_vec2().length() * cell_mm - cell_mm / 2.0).max(0.0))
        .collect()
}

/// A cell's place in a scatter: a hash of its coordinates (the `SplitMix64` finaliser).
fn scramble(cell: IVec2) -> u64 {
    let mut bits = (u64::from(cell.x as u32) << 32) | u64::from(cell.y as u32);
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    bits ^ (bits >> 31)
}
