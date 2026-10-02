use std::collections::HashMap;

use core_geometry::{
    Scalar, Vec3,
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
    /// Tiles the band crosses, each 512 quantised values with X running fastest.
    tiles: HashMap<IVec3, Box<[Quantised]>>,
    /// Per tile column, the inclusive ranges of tile Z that are wholly inside the solid.
    solid: HashMap<IVec2, Vec<(i32, i32)>>,
}

impl Sdf {
    pub(crate) fn new(
        grid: VoxelGrid,
        band_mm: Scalar,
        tiles: HashMap<IVec3, Box<[Quantised]>>,
        solid: HashMap<IVec2, Vec<(i32, i32)>>,
    ) -> Self {
        Self {
            grid,
            band_mm,
            step_mm: step_mm(band_mm),
            tiles,
            solid,
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

    pub(crate) fn tile_keys(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.tiles.keys().copied()
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
    fn sampling_between_two_voxels_lands_between_their_values() {
        let field = one_tile_field();
        let middle = field.sample(Vec3::new(0.5, 0.0, 0.0));
        assert!(
            (middle - 0.375).abs() < STEP,
            "halfway between -0.25 and 1.0 is 0.375, got {middle}"
        );
    }
}
