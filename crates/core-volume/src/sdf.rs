use std::collections::HashMap;

use core_geometry::{
    Aabb, FastMap, Scalar, Vec3,
    glam::{IVec2, IVec3},
};

use crate::grid::{TILE, VoxelGrid, index_in_tile, tile_of};

/// Values one tile holds.
pub(crate) const TILE_VALUES: usize = (TILE * TILE * TILE) as usize;

/// How a distance is held inside a tile: the band cut into `i16::MAX` steps either way.
///
/// The band is only ever a voxel or two wide, so a step is thousandths of a voxel — far
/// below anything marching cubes or a wall thickness can resolve — and a tile costs half
/// what the same values as `f32` cost; see
/// `docs/decisions/0065-a-fields-distances-are-quantised-to-its-band.md`.
pub(crate) type Quantised = i16;

/// What one step of a band of `band_mm` is worth, in millimetres.
pub(crate) fn step_mm(band_mm: Scalar) -> Scalar {
    band_mm / Scalar::from(Quantised::MAX)
}

/// `value_mm` as a step of the band, clamped to it.
pub(crate) fn encode(value_mm: Scalar, band_mm: Scalar) -> Quantised {
    let steps = (value_mm / band_mm).clamp(-1.0, 1.0) * Scalar::from(Quantised::MAX);
    steps.round() as Quantised
}

/// A signed distance field stored only where it says something: the tiles the surface
/// crosses, plus the runs of tiles that are wholly inside the solid.
///
/// Distances are millimetres, negative inside the surface, and clamped to the band beyond
/// it. Everything the field does not hold is the band's own value with the right sign; see
/// `docs/design/volume.md`.
#[derive(Debug, Clone)]
pub struct Sdf {
    grid: VoxelGrid,
    band_mm: Scalar,
    /// What one quantised step is worth, kept so that reading a value is a multiply
    /// rather than a divide: every lattice point of an extraction goes through it.
    step_mm: Scalar,
    /// Tiles the band crosses, each 512 quantised values with X running fastest. Keyed
    /// by a fast hash: an extraction looks one up for every lattice point.
    tiles: FastMap<IVec3, Box<[Quantised]>>,
    /// Per tile column, the inclusive ranges of tile Z that are wholly inside the solid.
    solid: FastMap<IVec2, Vec<(i32, i32)>>,
}

impl Sdf {
    pub(crate) fn new<S, T>(
        grid: VoxelGrid,
        band_mm: Scalar,
        tiles: HashMap<IVec3, Box<[Quantised]>, S>,
        solid: HashMap<IVec2, Vec<(i32, i32)>, T>,
    ) -> Self {
        Self {
            grid,
            band_mm,
            step_mm: step_mm(band_mm),
            tiles: tiles.into_iter().collect(),
            solid: solid.into_iter().collect(),
        }
    }

    pub fn grid(&self) -> VoxelGrid {
        self.grid
    }

    /// How far from the surface the field still carries a distance, in millimetres.
    pub fn band_mm(&self) -> Scalar {
        self.band_mm
    }

    /// Tiles the surface actually crosses. Not the volume: what is deep inside costs
    /// nothing to store.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// What holding the field costs, in bytes: its tiles and its runs, near enough to
    /// budget a cache by.
    pub fn bytes(&self) -> usize {
        let tile = TILE_VALUES * size_of::<Quantised>() + size_of::<(IVec3, Box<[Quantised]>)>();
        let runs: usize = self
            .solid
            .values()
            .map(|runs| size_of::<(IVec2, Vec<(i32, i32)>)>() + size_of_val(&runs[..]))
            .sum();
        self.tiles.len() * tile + runs
    }

    pub(crate) fn tile_keys(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.tiles.keys().copied()
    }

    /// Tiles the field does not store because they lie wholly inside the solid.
    pub(crate) fn inside_tiles(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.solid.iter().flat_map(|(column, runs)| {
            runs.iter()
                .flat_map(|(first, last)| *first..=*last)
                .map(|z| IVec3::new(column.x, column.y, z))
        })
    }

    /// Whether the surface or anything inside it may stand in `bounds`. Never false where
    /// it does; it may be true a tile and two voxels off.
    pub(crate) fn reaches(&self, bounds: &Aabb) -> bool {
        // Two voxels take in a surface vertex clustered past the tile its corners are in.
        let margin = Vec3::splat(2.0 * self.grid.voxel_mm);
        let first = tile_of(self.grid.voxel(bounds.mins - margin));
        let last = tile_of(self.grid.voxel(bounds.maxs + margin));
        (first.x..=last.x).any(|x| {
            (first.y..=last.y).any(|y| {
                let inside = self.solid.get(&IVec2::new(x, y)).is_some_and(|runs| {
                    runs.iter()
                        .any(|(bottom, top)| *bottom <= last.z && *top >= first.z)
                });
                inside || (first.z..=last.z).any(|z| self.tiles.contains_key(&IVec3::new(x, y, z)))
            })
        })
    }

    /// Value at a lattice point, in millimetres.
    pub fn value(&self, voxel: IVec3) -> Scalar {
        let tile = tile_of(voxel);
        if let Some(values) = self.tiles.get(&tile) {
            return Scalar::from(values[index_in_tile(voxel)]) * self.step_mm;
        }
        if self.is_solid_tile(tile) {
            -self.band_mm
        } else {
            self.band_mm
        }
    }

    /// The values of the `side`³ lattice points from `low`, a tile's lowest corner, with X
    /// running fastest, into `out`; `side` reaches at most one tile past `low`'s own.
    ///
    /// The same numbers as [`Self::value`] point by point, for eight tile lookups rather
    /// than one per point: an extraction reads every lattice point of the band this way.
    pub(crate) fn block(&self, low: IVec3, side: i32, out: &mut [Scalar]) {
        let first = tile_of(low);
        let around: [(Option<&[Quantised]>, Scalar); 8] = std::array::from_fn(|corner| {
            let step = |bit: usize| i32::from(corner & bit != 0);
            let tile = first + IVec3::new(step(1), step(2), step(4));
            let outside = if self.is_solid_tile(tile) {
                -self.band_mm
            } else {
                self.band_mm
            };
            (self.tiles.get(&tile).map(|values| &**values), outside)
        });
        for z in 0..side {
            for y in 0..side {
                for x in 0..side {
                    let voxel = low + IVec3::new(x, y, z);
                    let past = (voxel - first * TILE).div_euclid(IVec3::splat(TILE));
                    let (values, outside) = around[(past.x + 2 * past.y + 4 * past.z) as usize];
                    out[((z * side + y) * side + x) as usize] = values.map_or(outside, |values| {
                        Scalar::from(values[index_in_tile(voxel)]) * self.step_mm
                    });
                }
            }
        }
    }

    /// Value anywhere, by trilinear interpolation between the eight surrounding lattice
    /// points.
    pub fn sample(&self, point: Vec3) -> Scalar {
        let voxel = self.grid.voxel(point);
        let fraction = (point - self.grid.position(voxel)) / self.grid.voxel_mm;

        let mut total = 0.0;
        for corner in 0..8 {
            let step = IVec3::new(corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let weight =
                blend(fraction.x, step.x) * blend(fraction.y, step.y) * blend(fraction.z, step.z);
            total += weight * self.value(voxel + step);
        }
        total
    }

    /// Whether a tile the field does not store lies inside the solid.
    fn is_solid_tile(&self, tile: IVec3) -> bool {
        self.solid.get(&column_of(tile)).is_some_and(|runs| {
            runs.iter()
                .any(|(first, last)| tile.z >= *first && tile.z <= *last)
        })
    }
}

/// Tile column a tile stands in, which is what the solid runs are keyed by.
pub(crate) fn column_of(tile: IVec3) -> IVec2 {
    IVec2::new(tile.x, tile.y)
}

/// Works out which whole tiles lie inside the solid, from the tiles the surface crosses.
///
/// A column's solid stretches can only be the gaps between two stored tiles: a surface
/// passing anywhere else would have left a stored tile behind it. So each gap is decided
/// by one question, asked at its lowest tile.
pub(crate) fn gap_runs(
    stored: impl Iterator<Item = IVec3>,
    inside: impl Fn(IVec3) -> bool,
) -> HashMap<IVec2, Vec<(i32, i32)>> {
    let mut columns: HashMap<IVec2, Vec<i32>> = HashMap::new();
    for tile in stored {
        columns.entry(column_of(tile)).or_default().push(tile.z);
    }

    columns
        .into_iter()
        .filter_map(|(column, mut levels)| {
            levels.sort_unstable();
            let runs: Vec<(i32, i32)> = levels
                .windows(2)
                .filter(|pair| pair[1] > pair[0] + 1)
                .filter(|pair| inside(IVec3::new(column.x, column.y, pair[0] + 1)))
                .map(|pair| (pair[0] + 1, pair[1] - 1))
                .collect();
            (!runs.is_empty()).then_some((column, runs))
        })
        .collect()
}

/// Weight of one corner of a cell along one axis.
fn blend(fraction: Scalar, step: i32) -> Scalar {
    if step == 0 { 1.0 - fraction } else { fraction }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_tile_field() -> Sdf {
        let mut values = vec![encode(1.0, 1.0); TILE_VALUES];
        values[index_in_tile(IVec3::new(0, 0, 0))] = encode(-0.25, 1.0);
        let mut tiles = HashMap::new();
        tiles.insert(IVec3::ZERO, values.into_boxed_slice());

        let mut solid = HashMap::new();
        solid.insert(IVec2::new(0, 0), vec![(-3, -1)]);

        Sdf::new(VoxelGrid::new(1.0), 1.0, tiles, solid)
    }

    /// A value comes back within one step of the band it was quantised to, which on a
    /// band of a millimetre is thirty microns of a micron.
    const STEP: Scalar = 1.0 / Quantised::MAX as Scalar;

    #[test]
    fn a_stored_voxel_answers_with_its_own_value() {
        let field = one_tile_field();
        assert!((field.value(IVec3::ZERO) + 0.25).abs() < STEP);
        assert!((field.value(IVec3::new(1, 0, 0)) - 1.0).abs() < STEP);
    }

    #[test]
    fn a_tile_inside_the_solid_answers_with_the_band_made_negative() {
        let field = one_tile_field();
        assert!((field.value(IVec3::new(0, 0, -9)) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_tile_the_field_never_saw_is_outside() {
        let field = one_tile_field();
        assert!((field.value(IVec3::new(400, 0, 0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_box_reaches_the_field_through_its_band_or_its_inside_and_nowhere_else() {
        // Tiles are 8 mm on this 1 mm lattice: the band fills z 0..8, the inside z -24..0.
        let field = one_tile_field();
        let cube = |at: Vec3| Aabb::new(at, at + Vec3::ONE);
        assert!(field.reaches(&cube(Vec3::new(2.0, 2.0, 4.0))), "the band");
        assert!(
            field.reaches(&cube(Vec3::new(2.0, 2.0, -20.0))),
            "the inside"
        );
        assert!(!field.reaches(&cube(Vec3::new(100.0, 2.0, 4.0))), "far off");
        assert!(
            !field.reaches(&cube(Vec3::new(2.0, 2.0, -40.0))),
            "under it"
        );
    }

    #[test]
    fn sampling_between_two_voxels_lands_between_their_values() {
        let field = one_tile_field();
        let middle = field.sample(Vec3::new(0.5, 0.0, 0.0));
        assert!(
            (middle - 0.375).abs() < STEP,
            "halfway between -0.25 and 1.0 is 0.375, got {middle}"
        );
    }

    #[test]
    fn a_block_reads_the_same_values_as_one_point_at_a_time() {
        let band_mm = 1.0;
        let ramp = |tile: i32| -> Box<[Quantised]> {
            (0..TILE_VALUES)
                .map(|index| encode((index as Scalar + tile as Scalar) / 600.0 - 0.5, band_mm))
                .collect()
        };
        // Two stored tiles side by side in X, and a solid run under them.
        let field = Sdf::new(
            VoxelGrid::new(0.1),
            band_mm,
            HashMap::from([(IVec3::ZERO, ramp(0)), (IVec3::X, ramp(7))]),
            HashMap::from([(IVec2::ZERO, vec![(-3, -1)])]),
        );
        for low in [IVec3::ZERO, IVec3::new(0, 0, -TILE), IVec3::new(TILE, 0, 0)] {
            let side = TILE + 1;
            let mut block = vec![0.0; (side * side * side) as usize];
            field.block(low, side, &mut block);
            for z in 0..side {
                for y in 0..side {
                    for x in 0..side {
                        let voxel = low + IVec3::new(x, y, z);
                        let read = block[((z * side + y) * side + x) as usize];
                        assert_eq!(read, field.value(voxel), "at {voxel}");
                    }
                }
            }
        }
    }
}
