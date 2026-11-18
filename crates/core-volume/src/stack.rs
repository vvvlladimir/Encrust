//! A field built back from a stack of layer masks, so a sliced file becomes a model again.
//! How the cover of a voxel stands in for a distance: `docs/design/volume.md`.

use std::collections::HashMap;

use core_geometry::{
    Scalar,
    glam::{IVec2, IVec3},
};
use rayon::prelude::*;

use crate::grid::{TILE, VoxelGrid};
use crate::sdf::{Quantised, Sdf, TILE_VALUES, encode};

/// Cover of a cell held as a byte; only an exact edge of it is wholly empty or wholly lit.
const FULL: u8 = u8::MAX;

/// Cells, or lattice points, along a side of a block: a tile wide, so a block of points is
/// the foot of one tile column and a slab is touched only where its blocks are lit.
const BLOCK: usize = TILE as usize;
const BLOCK_POINTS: usize = BLOCK * BLOCK;

/// The panel a stack was exposed on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub width_px: u32,
    pub height_px: u32,
    /// Pixel pitch along X and along Y, millimetres.
    pub pitch_mm: (Scalar, Scalar),
}

/// The field of a stack fed to it one layer at a time, bottom first, holding a tile's height
/// of lattice planes at most; panel pixel `(0, 0)` is the origin and Y runs with the rows.
pub struct StackField {
    grid: VoxelGrid,
    cells: Cells,
    /// Cover of the cell slab being filled, a fraction of each cell, which slab it is and
    /// the blocks of cells it lights, sorted.
    filling: Vec<f32>,
    filling_lit: Vec<usize>,
    filling_at: i32,
    /// The slab below it, finished, and the blocks it lights.
    below: Vec<u8>,
    below_lit: Vec<usize>,
    /// A cleared slab, which the next one to finish is written into.
    spare: Vec<u8>,
    /// Lattice planes of the tile row being built, the lowest first.
    planes: Vec<Plane>,
    tiles: HashMap<IVec3, Box<[Quantised]>>,
    solid: HashMap<IVec2, Vec<(i32, i32)>>,
}

/// How the panel's pixels fall into voxel cells, each by its centre.
struct Cells {
    across: usize,
    along: usize,
    of_x: Vec<u32>,
    of_y: Vec<u32>,
    /// First pixel column of the next cell, for each cell.
    end_x: Vec<u32>,
    /// Pixels in each cell column and in each cell row.
    count_x: Vec<u32>,
    count_y: Vec<u32>,
    width_px: u32,
}

impl Cells {
    fn new(panel: Panel, voxel_mm: Scalar) -> Self {
        let (of_x, count_x) = assign(panel.width_px, panel.pitch_mm.0, voxel_mm);
        let (of_y, count_y) = assign(panel.height_px, panel.pitch_mm.1, voxel_mm);
        let mut end_x = vec![0; count_x.len()];
        for (px, cell) in of_x.iter().enumerate() {
            end_x[*cell as usize] = px as u32 + 1;
        }
        Self {
            across: count_x.len(),
            along: count_y.len(),
            of_x,
            of_y,
            end_x,
            count_x,
            count_y,
            width_px: panel.width_px,
        }
    }

    fn len(&self) -> usize {
        self.across * self.along
    }

    /// The pixel rows whose centres fall in cell row `cell_row`.
    fn rows_of(&self, cell_row: usize) -> std::ops::Range<usize> {
        let first = self
            .of_y
            .partition_point(|cell| (*cell as usize) < cell_row);
        first..first + self.count_y[cell_row] as usize
    }

    /// Blocks of cells across the panel.
    fn blocks_across(&self) -> usize {
        self.across.div_ceil(BLOCK)
    }

    /// Blocks of lattice points across and along the panel, one point past its last cell.
    fn point_blocks(&self) -> (usize, usize) {
        (
            (self.across + 1).div_ceil(BLOCK),
            (self.along + 1).div_ceil(BLOCK),
        )
    }

    /// The cover of cell `(x - 1, y - 1)` of `slab`, nothing off the panel.
    fn cover(&self, slab: &[u8], x: usize, y: usize) -> u32 {
        if x == 0 || y == 0 || x > self.across || y > self.along {
            return 0;
        }
        u32::from(slab[(y - 1) * self.across + (x - 1)])
    }
}

/// The cell of each of `count` pixels of `pitch_mm`, and how many pixels each cell holds.
fn assign(count: u32, pitch_mm: Scalar, voxel_mm: Scalar) -> (Vec<u32>, Vec<u32>) {
    let of: Vec<u32> = (0..count)
        .map(|px| ((px as Scalar + 0.5) * pitch_mm / voxel_mm).floor() as u32)
        .collect();
    let mut counts = vec![0; of.last().map_or(0, |last| *last as usize + 1)];
    for cell in &of {
        counts[*cell as usize] += 1;
    }
    (of, counts)
}

impl StackField {
    /// A field over `panel` with lattice points `voxel_mm` apart, or a pixel apart where
    /// that is coarser: a cell smaller than a pixel would hold none.
    pub fn new(panel: Panel, voxel_mm: Scalar) -> Self {
        let voxel_mm = voxel_mm.max(panel.pitch_mm.0).max(panel.pitch_mm.1);
        let cells = Cells::new(panel, voxel_mm);
        let size = cells.len();
        Self {
            grid: VoxelGrid::new(voxel_mm),
            cells,
            filling: vec![0.0; size],
            filling_lit: Vec::new(),
            filling_at: 0,
            below: vec![0; size],
            below_lit: Vec::new(),
            spare: vec![0; size],
            planes: Vec::with_capacity(BLOCK),
            tiles: HashMap::new(),
            solid: HashMap::new(),
        }
    }

    /// Adds the layer from `bottom_mm` to `top_mm` above the plate, as runs of `(length,
    /// grey)` over the panel row by row. Layers come bottom first and do not overlap.
    pub fn push(&mut self, bottom_mm: Scalar, top_mm: Scalar, runs: &[(u32, u8)]) {
        let voxel_mm = self.grid.voxel_mm;
        while (self.filling_at + 1) as Scalar * voxel_mm <= bottom_mm {
            self.close_slab();
        }
        let mut from = bottom_mm;
        loop {
            let slab_top = (self.filling_at + 1) as Scalar * voxel_mm;
            let to = top_mm.min(slab_top);
            if to > from {
                self.add(runs, (to - from) / voxel_mm);
            }
            if top_mm <= slab_top {
                break;
            }
            self.close_slab();
            from = slab_top;
        }
    }

    /// The field of everything pushed, closed over its top.
    pub fn finish(mut self) -> Sdf {
        // The slab being filled, then an empty one so the top has a plane above it.
        self.close_slab();
        self.close_slab();
        if !self.planes.is_empty() {
            self.flush_row();
        }
        Sdf::new(self.grid, self.grid.voxel_mm, self.tiles, self.solid)
    }

    /// Spreads a layer's runs over the cells their pixels fall in, `weight` of a slab deep,
    /// each row of blocks on its own thread.
    fn add(&mut self, runs: &[(u32, u8)], weight: f32) {
        let cells = &self.cells;
        let starts = row_starts(runs, cells.width_px, cells.of_y.len());
        let blocks_across = cells.blocks_across();
        let lit: Vec<Vec<usize>> = self
            .filling
            .par_chunks_mut(BLOCK * cells.across)
            .enumerate()
            .map(|(row, filling)| {
                let mut lit = vec![false; blocks_across];
                for (in_row, filling) in filling.chunks_mut(cells.across).enumerate() {
                    let cell_row = row * BLOCK + in_row;
                    for pixel_row in cells.rows_of(cell_row) {
                        let (first_run, skipped) = starts[pixel_row];
                        for (first, last, grey) in
                            lit_spans(runs, first_run, skipped, cells.width_px)
                        {
                            spread(filling, cells, cell_row, (first, last), grey, weight);
                            let first = cells.of_x[first as usize] as usize / BLOCK;
                            let last = cells.of_x[last as usize] as usize / BLOCK;
                            lit[first..=last].fill(true);
                        }
                    }
                }
                let at = row * blocks_across;
                (0..blocks_across)
                    .filter(|block| lit[*block])
                    .map(|block| at + block)
                    .collect()
            })
            .collect();
        // A slab thicker than the layer takes several, each lighting blocks the others may.
        let merge = !self.filling_lit.is_empty();
        self.filling_lit.extend(lit.into_iter().flatten());
        if merge {
            self.filling_lit.sort_unstable();
            self.filling_lit.dedup();
        }
    }

    /// Finishes the slab being filled and the lattice plane at its foot, touching only the
    /// blocks either slab lights.
    fn close_slab(&mut self) {
        let (across, blocks_across) = (self.cells.across, self.cells.blocks_across());
        let lit = &self.filling_lit;
        self.filling
            .par_chunks_mut(BLOCK * across)
            .zip(self.spare.par_chunks_mut(BLOCK * across))
            .enumerate()
            .for_each(|(row, (filling, spare))| {
                for index in lit_cells(lit, row, blocks_across, across, filling.len()) {
                    spare[index] = as_byte(filling[index]);
                    filling[index] = 0.0;
                }
            });
        let plane = lattice_plane(
            &self.cells,
            [&self.below, &self.spare],
            [&self.below_lit, &self.filling_lit],
        );

        // The slab below is done with; cleared, its buffer takes the next slab.
        let lit = &self.below_lit;
        self.below
            .par_chunks_mut(BLOCK * across)
            .enumerate()
            .for_each(|(row, below)| {
                for index in lit_cells(lit, row, blocks_across, across, below.len()) {
                    below[index] = 0;
                }
            });
        std::mem::swap(&mut self.below, &mut self.spare);
        std::mem::swap(&mut self.below_lit, &mut self.filling_lit);
        self.filling_lit.clear();

        self.planes.push(plane);
        self.filling_at += 1;
        if self.planes.len() == BLOCK {
            self.flush_row();
        }
    }

    /// Turns the planes of one tile row into stored tiles and solid runs.
    fn flush_row(&mut self) {
        let row = (self.filling_at - 1).div_euclid(TILE);
        let mut columns: Vec<usize> = self
            .planes
            .iter()
            .flat_map(|plane| plane.blocks.iter().map(|(index, _)| *index))
            .collect();
        columns.sort_unstable();
        columns.dedup();
        let across = self.cells.point_blocks().0;
        let made: Vec<(usize, Tile)> = columns
            .par_iter()
            .map(|&index| (index, tile(&self.planes, index, self.grid.voxel_mm)))
            .collect();
        for (index, made) in made {
            let column = IVec2::new(as_tile(index % across), as_tile(index / across));
            match made {
                Tile::Outside => {}
                Tile::Inside => extend_run(self.solid.entry(column).or_default(), row),
                Tile::Stored(values) => {
                    self.tiles.insert(column.extend(row), values);
                }
            }
        }
        self.planes.clear();
    }
}

/// A block's place along one axis as a tile's. A panel's pixel count is bounded well inside
/// `i32`, so the saturation is never reached.
fn as_tile(block: usize) -> i32 {
    i32::try_from(block).unwrap_or(i32::MAX)
}

/// Where in a block row `row` of cells, `rows` cells long, the cells of its lit blocks
/// sit; `lit` holds every lit block of the slab, sorted.
fn lit_cells(
    lit: &[usize],
    row: usize,
    blocks_across: usize,
    across: usize,
    rows: usize,
) -> impl Iterator<Item = usize> {
    let first = lit.partition_point(|block| *block < row * blocks_across);
    let last = lit.partition_point(|block| *block < (row + 1) * blocks_across);
    lit[first..last].iter().flat_map(move |block| {
        let x0 = block % blocks_across * BLOCK;
        let x1 = (x0 + BLOCK).min(across);
        (0..rows / across).flat_map(move |y| (x0..x1).map(move |x| y * across + x))
    })
}

/// One lattice plane, held only over the blocks of points a lit cell touches, sorted.
struct Plane {
    blocks: Vec<(usize, [u8; BLOCK_POINTS])>,
}

impl Plane {
    fn block(&self, index: usize) -> Option<&[u8; BLOCK_POINTS]> {
        let at = self.blocks.binary_search_by_key(&index, |(at, _)| *at);
        at.ok().map(|at| &self.blocks[at].1)
    }
}

/// Adds the pixels `first..=last` of one pixel row, lit at `grey`, to `filling`, the cells
/// of row `cell_row`.
fn spread(
    filling: &mut [f32],
    cells: &Cells,
    cell_row: usize,
    (first, last): (u32, u32),
    grey: u8,
    weight: f32,
) {
    let row_pixels = cells.count_y[cell_row] as f32;
    let lit = f32::from(grey) / f32::from(FULL) * weight / row_pixels;
    let mut px = first;
    while px <= last {
        let cell = cells.of_x[px as usize] as usize;
        let to = cells.end_x[cell].min(last + 1);
        filling[cell] += lit * (to - px) as f32 / cells.count_x[cell] as f32;
        px = to;
    }
}

/// For each pixel row, the run it begins in and how many of that run's pixels lie before it.
fn row_starts(runs: &[(u32, u8)], width_px: u32, rows: usize) -> Vec<(usize, u64)> {
    let width = u64::from(width_px);
    let mut starts = Vec::with_capacity(rows);
    let mut at = 0u64;
    for (index, &(length, _)) in runs.iter().enumerate() {
        let end = at + u64::from(length);
        while starts.len() < rows && (starts.len() as u64) * width < end {
            starts.push((index, starts.len() as u64 * width - at));
        }
        at = end;
    }
    starts.resize(rows, (runs.len(), 0));
    starts
}

/// The lit spans of the pixel row beginning `skipped` pixels into run `first_run`: first and
/// last pixel column, and the grey.
fn lit_spans(
    runs: &[(u32, u8)],
    first_run: usize,
    skipped: u64,
    width_px: u32,
) -> impl Iterator<Item = (u32, u32, u8)> + '_ {
    let width = u64::from(width_px);
    let mut x = 0u64;
    let mut skip = skipped;
    runs.get(first_run..)
        .unwrap_or_default()
        .iter()
        .map_while(move |&(length, grey)| {
            if x >= width {
                return None;
            }
            let length = (u64::from(length) - skip).min(width - x);
            skip = 0;
            if length == 0 {
                // An empty run, which a hostile file may carry, lights nothing.
                return Some((0, 0, 0));
            }
            let span = (x as u32, (x + length - 1) as u32, grey);
            x += length;
            Some(span)
        })
        .filter(|&(_, _, grey)| grey > 0)
}

/// A cover as a byte, wholly empty or wholly lit only where it is so to within rounding.
fn as_byte(cover: f32) -> u8 {
    // Layers of 35 µm summed into 50 µm slabs carry a few ulps either way.
    match cover {
        c if c >= 0.999 => FULL,
        c if c <= 0.001 => 0,
        c => (c * f32::from(FULL))
            .round()
            .clamp(1.0, f32::from(FULL - 1)) as u8,
    }
}

/// Cover at the lattice points about the lit blocks of `slabs`, below then above: the mean
/// of the eight cells about each point, so a surface never passes between two saturated ones.
fn lattice_plane(cells: &Cells, slabs: [&[u8]; 2], lit: [&[usize]; 2]) -> Plane {
    let blocks_across = cells.blocks_across();
    let (across, along) = cells.point_blocks();
    // A point touches the cells at and before it, so a lit block reaches one block further.
    let mut wanted: Vec<usize> = lit
        .iter()
        .flat_map(|lit| lit.iter())
        .flat_map(|block| {
            let (x, y) = (block % blocks_across, block / blocks_across);
            [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)]
        })
        .filter(|&(x, y)| x < across && y < along)
        .map(|(x, y)| y * across + x)
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    let blocks = wanted
        .par_iter()
        .filter_map(|&index| {
            let (x0, y0) = (index % across * BLOCK, index / across * BLOCK);
            let mut values = [0; BLOCK_POINTS];
            for (local, point) in values.iter_mut().enumerate() {
                let (x, y) = (x0 + local % BLOCK, y0 + local / BLOCK);
                let sum: u32 = slabs
                    .iter()
                    .map(|slab| {
                        cells.cover(slab, x, y)
                            + cells.cover(slab, x + 1, y)
                            + cells.cover(slab, x, y + 1)
                            + cells.cover(slab, x + 1, y + 1)
                    })
                    .sum();
                *point = mean_of_eight(sum);
            }
            values
                .iter()
                .any(|point| *point > 0)
                .then_some((index, values))
        })
        .collect();
    Plane { blocks }
}

/// The mean of eight bytes summing to `sum`, full or empty only when all eight are.
fn mean_of_eight(sum: u32) -> u8 {
    match sum {
        0 => 0,
        s if s == 8 * u32::from(FULL) => FULL,
        s => ((s + 4) / 8).clamp(1, u32::from(FULL - 1)) as u8,
    }
}

/// What one tile of a row turned out to be.
enum Tile {
    Outside,
    Inside,
    Stored(Box<[Quantised]>),
}

/// The tile over the block of points `index` in the row `planes` make, a voxel `band_mm`.
fn tile(planes: &[Plane], index: usize, band_mm: Scalar) -> Tile {
    // A block's points run along X then Y, as a tile's do within each of its planes.
    let mut covers = [0; TILE_VALUES];
    for (plane, covers) in planes.iter().zip(covers.as_chunks_mut::<BLOCK_POINTS>().0) {
        if let Some(block) = plane.block(index) {
            covers.copy_from_slice(block);
        }
    }
    let lit = covers.iter().any(|cover| *cover > 0);
    let dark = covers.iter().any(|cover| *cover < FULL);
    match (lit, dark) {
        (false, _) => Tile::Outside,
        (true, false) => Tile::Inside,
        (true, true) => Tile::Stored(
            covers
                .iter()
                // A box two voxels wide: half lit is the surface, saturation a voxel either side.
                .map(|cover| {
                    let distance = (0.5 - f32::from(*cover) / f32::from(FULL)) * 2.0 * band_mm;
                    encode(distance, band_mm)
                })
                .collect(),
        ),
    }
}

/// Adds tile `row` to a column's solid runs, the rows coming in rising order.
fn extend_run(runs: &mut Vec<(i32, i32)>, row: i32) {
    match runs.last_mut() {
        Some((_, last)) if *last + 1 == row => *last = row,
        _ => runs.push((row, row)),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{diagnose, signed_volume};

    use super::*;
    use crate::{Cancel, extract};

    /// A panel of 0.05 mm pixels, 200 x 160 of them: 10 x 8 mm.
    const PANEL: Panel = Panel {
        width_px: 200,
        height_px: 160,
        pitch_mm: (0.05, 0.05),
    };

    /// Runs over `PANEL` lighting the pixels `x0..x1` of the rows `y0..y1`.
    fn block_runs((x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> Vec<(u32, u8)> {
        let width = PANEL.width_px;
        let mut runs = vec![(y0 * width + x0, 0)];
        for y in y0..y1 {
            runs.push((x1 - x0, 255));
            let gap = if y + 1 < y1 {
                width - (x1 - x0)
            } else {
                width - x1 + (PANEL.height_px - y1) * width
            };
            runs.push((gap, 0));
        }
        runs
    }

    fn stacked(runs: &[(u32, u8)], layer_mm: Scalar, layers: u32, voxel_mm: Scalar) -> Sdf {
        let mut field = StackField::new(PANEL, voxel_mm);
        for layer in 0..layers {
            let bottom = layer as Scalar * layer_mm;
            field.push(bottom, bottom + layer_mm, runs);
        }
        field.finish()
    }

    #[test]
    fn a_block_of_lit_layers_comes_back_a_closed_box_of_its_volume() {
        // 80 x 80 pixels of 0.05 mm is 4 x 4 mm, 80 layers of 0.05 mm another 4 mm.
        let field = stacked(&block_runs((60, 140), (40, 120)), 0.05, 80, 0.1);
        let mesh = extract(&field, Cancel::never());

        assert!(
            diagnose(&mesh).is_closed(),
            "a stack closes over its top and its foot"
        );
        let volume = signed_volume(&mesh);
        // A box filter two voxels wide rounds each of the twelve edges by about a voxel.
        assert!(
            (volume - 64.0).abs() < 64.0 * 0.05,
            "a 4 mm cube holds 64 mm³, got {volume}"
        );
    }

    #[test]
    fn a_block_comes_back_where_its_pixels_were() {
        let field = stacked(&block_runs((60, 140), (40, 120)), 0.05, 80, 0.1);
        let bounds = extract(&field, Cancel::never())
            .aabb()
            .expect("a block has a mesh");
        // Pixel 60 starts 3 mm in, row 40 at 2 mm; the top is 80 layers of 0.05 mm.
        let tolerance = 0.1;
        assert!((bounds.mins.x - 3.0).abs() < tolerance, "{:?}", bounds.mins);
        assert!((bounds.mins.y - 2.0).abs() < tolerance, "{:?}", bounds.mins);
        assert!(bounds.mins.z.abs() < tolerance, "{:?}", bounds.mins);
        assert!((bounds.maxs.z - 4.0).abs() < tolerance, "{:?}", bounds.maxs);
    }

    #[test]
    fn layers_thicker_than_a_voxel_fill_every_slab_they_span() {
        // Ten layers of 0.4 mm over voxels of 0.1 mm: the same 4 mm cube.
        let field = stacked(&block_runs((60, 140), (40, 120)), 0.4, 10, 0.1);
        let mesh = extract(&field, Cancel::never());
        let volume = signed_volume(&mesh);
        assert!((volume - 64.0).abs() < 64.0 * 0.05, "got {volume}");
    }

    #[test]
    fn a_voxel_finer_than_a_pixel_is_taken_at_the_pixel() {
        // 0.05 mm pixels asked for at 0.02 mm: every cell must still hold one.
        let field = stacked(&block_runs((60, 140), (40, 120)), 0.05, 80, 0.02);
        assert_eq!(field.grid().voxel_mm, 0.05);
        let volume = signed_volume(&extract(&field, Cancel::never()));
        assert!((volume - 64.0).abs() < 64.0 * 0.05, "got {volume}");
    }

    #[test]
    fn an_unlit_stack_is_no_field_at_all() {
        let dark = vec![(PANEL.width_px * PANEL.height_px, 0)];
        let field = stacked(&dark, 0.05, 10, 0.1);
        assert!(field.is_empty());
        assert!(extract(&field, Cancel::never()).faces.is_empty());
    }

    #[test]
    fn a_cover_is_full_or_empty_only_to_within_rounding() {
        assert_eq!(as_byte(0.7 + 0.3), FULL);
        assert_eq!(as_byte(0.0004), 0);
        assert_eq!(as_byte(0.002), 1);
        assert_eq!(mean_of_eight(8 * 255 - 1), FULL - 1);
        assert_eq!(mean_of_eight(1), 1);
    }
}
