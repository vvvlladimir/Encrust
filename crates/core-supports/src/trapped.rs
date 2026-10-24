use core_geometry::{Aabb, FastMap, Scalar, Vec2, Vec3};
use core_slicer::Layer;

use crate::config::GRID_PITCH_MM;
use crate::field::{Field, Grid, Span};

/// A pocket smaller than this is a droplet the wash takes out, not a print failure.
const MIN_TRAPPED_MM3: Scalar = 5.0;

/// Resin that cannot get out: a pocket of air the stack closes over without it ever
/// reaching the outside of the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trapped {
    /// Where a hole goes: the middle of the widest run of the pocket's lowest layer, in
    /// plate millimetres, which is a point certainly inside the pocket.
    pub at: Vec3,
    /// What the pocket holds, cubic millimetres.
    pub volume_mm3: Scalar,
    /// The box it stands in, plate millimetres, out to the edge of the cells it filled.
    /// What marks this pocket on screen and nothing else; see ADR 0200.
    pub bounds: Aabb,
}

/// Finds the trapped resin in a stack, a layer at a time in print order, holding one layer of air
/// and the open pockets, never the stack; see `docs/design/hollowing.md`.
#[derive(Debug)]
pub struct TrapScan {
    grid: Grid,
    layer_height_mm: Scalar,
    /// Pockets still growing, by the id `previous` knows them by; freed slots are reused, so this
    /// is as long as the pockets alive at once, not the stack.
    pockets: Vec<Option<Pocket>>,
    free: Vec<usize>,
    /// The air of the layer below, in row-major order, with the pocket each run belongs to.
    previous: Vec<Air>,
    layers: usize,
    found: Vec<Trapped>,
}

/// One run of air on one row, and the pocket it belongs to.
#[derive(Debug, Clone, Copy)]
struct Air {
    row: i32,
    x0: i32,
    x1: i32,
    pocket: usize,
}

impl Air {
    fn overlaps(self, other: Self) -> bool {
        self.x0 < other.x1 && other.x0 < self.x1
    }

    fn width(self) -> i32 {
        self.x1 - self.x0
    }
}

#[derive(Debug)]
struct Pocket {
    cells: u64,
    /// The cells it has reached, and the heights it was seen at: `x0`..`x1` across,
    /// `row` up, in the grid's own numbering.
    box_low: Air,
    box_high: Air,
    low_z: Scalar,
    high_z: Scalar,
    /// The pocket has reached the edge of the grid or the plate, so the resin in it has
    /// somewhere to go.
    open: bool,
    /// The layer the pocket opened on, counted from the bottom of the stack, and how high
    /// that was.
    floor_layer: usize,
    floor_z: Scalar,
    /// The widest run of that lowest layer.
    floor: Air,
}

impl TrapScan {
    /// A scan over a model whose footprint spans `min`..`max` in plate millimetres.
    pub fn new(min: Vec2, max: Vec2, layer_height_mm: Scalar) -> Self {
        Self {
            grid: Grid::covering(min, max, GRID_PITCH_MM),
            layer_height_mm,
            pockets: Vec::new(),
            free: Vec::new(),
            previous: Vec::new(),
            layers: 0,
            found: Vec::new(),
        }
    }

    /// Reads the next layer up. Layers must arrive in print order, bottom first.
    pub fn push(&mut self, layer: &Layer) {
        let field = Field::of_layer(layer, &self.grid);
        let (air, rows) = self.air_of(&field);
        let mut runs = Runs::of(&air, &rows);

        let mut alias = FastMap::default();
        let assigned = self.adopt(&mut runs, &air, &rows, &mut alias);
        let mut seen = vec![false; self.pockets.len()];
        let mut current = Vec::with_capacity(air.len());

        for (index, run) in air.iter().enumerate() {
            let root = runs.find(index);
            let pocket = resolve(&alias, assigned[root]);
            self.absorb(pocket, *run, layer.z, self.layers);
            seen[pocket] = true;
            current.push(Air { pocket, ..*run });
        }

        self.centre_floors(&current);
        self.close_unseen(&seen);
        // A slot freed by a merge is handed out only once this layer is read: its alias
        // would otherwise fold a pocket opened on the same layer into the survivor.
        self.free.extend(alias.keys());
        self.previous = current;
        self.layers += 1;
    }

    /// Everything trapped in the stack that was read; a pocket still growing at the top layer is
    /// open to the air, so what is left alive is dropped.
    pub fn finish(self) -> Vec<Trapped> {
        self.found
    }

    /// The air of one layer: the complement of the material, row by row over the whole
    /// grid, so the cells around the model are air like any other.
    fn air_of(&self, field: &Field) -> (Vec<Air>, Vec<Row>) {
        let columns = self.grid.columns();
        let mut material = field.rows().peekable();
        let mut air = Vec::new();
        let mut rows = Vec::new();

        for row in self.grid.rows() {
            while material.peek().is_some_and(|&(at, _)| at < row) {
                material.next();
            }
            let solid: &[Span] = match material.peek() {
                Some(&(at, spans)) if at == row => spans,
                _ => &[],
            };

            let start = air.len();
            let mut cursor = columns.start;
            for span in solid {
                if span.x0 > cursor {
                    air.push(Air::new(row, cursor, span.x0));
                }
                cursor = cursor.max(span.x1);
            }
            if cursor < columns.end {
                air.push(Air::new(row, cursor, columns.end));
            }
            if air.len() > start {
                rows.push(Row {
                    at: row,
                    start,
                    end: air.len(),
                });
            }
        }
        (air, rows)
    }

    /// Gives every run of this layer the pocket it continues, merging two pockets this
    /// layer joins and opening a new one where nothing below answers.
    fn adopt(
        &mut self,
        runs: &mut Runs,
        air: &[Air],
        rows: &[Row],
        alias: &mut FastMap<usize, usize>,
    ) -> Vec<usize> {
        let mut assigned = vec![usize::MAX; air.len()];
        for (index, below) in links(air, rows, &self.previous) {
            let root = runs.find(index);
            let below = resolve(alias, below);
            let held = resolve(alias, assigned[root]);
            assigned[root] = if held == usize::MAX {
                below
            } else {
                self.merge(held, below, alias)
            };
        }

        for index in 0..air.len() {
            let root = runs.find(index);
            if assigned[root] == usize::MAX {
                assigned[root] = self.open_pocket();
            }
        }
        assigned
    }

    /// Folds `dead` into `into` and leaves a forwarding entry behind, so the runs already
    /// pointing at the dead pocket still land on the survivor. The slot is freed by `push`.
    fn merge(&mut self, into: usize, dead: usize, alias: &mut FastMap<usize, usize>) -> usize {
        if into == dead {
            return into;
        }
        let Some(gone) = self.pockets[dead].take() else {
            return into;
        };
        let Some(kept) = self.pockets[into].as_mut() else {
            return into;
        };
        kept.cells += gone.cells;
        kept.open |= gone.open;
        kept.box_low = Air::new(
            kept.box_low.row.min(gone.box_low.row),
            kept.box_low.x0.min(gone.box_low.x0),
            kept.box_low.x1.min(gone.box_low.x1),
        );
        kept.box_high = Air::new(
            kept.box_high.row.max(gone.box_high.row),
            kept.box_high.x0.max(gone.box_high.x0),
            kept.box_high.x1.max(gone.box_high.x1),
        );
        kept.low_z = kept.low_z.min(gone.low_z);
        kept.high_z = kept.high_z.max(gone.high_z);
        let deeper = gone.floor_layer < kept.floor_layer;
        let wider = gone.floor_layer == kept.floor_layer && gone.floor.width() > kept.floor.width();
        if deeper || wider {
            kept.floor_layer = gone.floor_layer;
            kept.floor_z = gone.floor_z;
            kept.floor = gone.floor;
        }

        alias.insert(dead, into);
        into
    }

    fn open_pocket(&mut self) -> usize {
        let pocket = Pocket {
            cells: 0,
            box_low: Air::new(i32::MAX, i32::MAX, i32::MAX),
            box_high: Air::new(i32::MIN, i32::MIN, i32::MIN),
            low_z: Scalar::INFINITY,
            high_z: Scalar::NEG_INFINITY,
            // The first layer of a stack stands on the plate, so air inside it drains
            // straight out onto it.
            open: self.layers == 0,
            floor_layer: self.layers,
            floor_z: Scalar::INFINITY,
            floor: Air::new(0, 0, 0),
        };
        if let Some(slot) = self.free.pop() {
            self.pockets[slot] = Some(pocket);
            return slot;
        }
        self.pockets.push(Some(pocket));
        self.pockets.len() - 1
    }

    /// Adds one run of air to the pocket it belongs to.
    fn absorb(&mut self, pocket: usize, run: Air, z: Scalar, layer: usize) {
        let (columns, rows) = (self.grid.columns(), self.grid.rows());
        let on_edge = run.x0 <= columns.start
            || run.x1 >= columns.end
            || run.row <= rows.start
            || run.row >= rows.end - 1;

        let Some(pocket) = self.pockets[pocket].as_mut() else {
            return;
        };
        pocket.cells += run.width().max(0) as u64;
        pocket.open |= on_edge;
        pocket.box_low = Air::new(
            pocket.box_low.row.min(run.row),
            pocket.box_low.x0.min(run.x0),
            pocket.box_low.x1.min(run.x1),
        );
        pocket.box_high = Air::new(
            pocket.box_high.row.max(run.row),
            pocket.box_high.x0.max(run.x0),
            pocket.box_high.x1.max(run.x1),
        );
        pocket.low_z = pocket.low_z.min(z);
        pocket.high_z = pocket.high_z.max(z);
        if pocket.floor_layer == layer {
            pocket.floor_z = z;
            if run.width() > pocket.floor.width() {
                pocket.floor = run;
            }
        }
    }

    /// Puts a new pocket's hole in the middle of its floor rather than on its rim: the
    /// widest run of the middle row of the runs it opened on.
    fn centre_floors(&mut self, current: &[Air]) {
        let mut rows: FastMap<usize, (i32, i32)> = FastMap::default();
        for run in current {
            let new = self.pockets[run.pocket]
                .as_ref()
                .is_some_and(|pocket| pocket.floor_layer == self.layers);
            if new {
                let range = rows.entry(run.pocket).or_insert((run.row, run.row));
                range.0 = range.0.min(run.row);
                range.1 = range.1.max(run.row);
            }
        }

        for run in current {
            let Some(&(low, high)) = rows.get(&run.pocket) else {
                continue;
            };
            let Some(pocket) = self.pockets[run.pocket].as_mut() else {
                continue;
            };
            let middle = low + (high - low) / 2;
            if run.row == middle
                && (pocket.floor.row != middle || run.width() > pocket.floor.width())
            {
                pocket.floor = *run;
            }
        }
    }

    /// The box one pocket fills, plate millimetres: the cells it reached out to their
    /// own edges, and half a layer past the heights it was seen at.
    fn box_of(&self, pocket: &Pocket) -> Aabb {
        let half = self.layer_height_mm / 2.0;
        let low = self.grid.centre_mm(pocket.box_low.x0, pocket.box_low.row);
        let high = self.grid.centre_mm(pocket.box_high.x1, pocket.box_high.row);
        let cell = self.grid.millimetres_of(1) / 2.0;
        Aabb::new(
            Vec3::new(low.x - cell, low.y - cell, pocket.low_z - half),
            Vec3::new(high.x + cell, high.y + cell, pocket.high_z + half),
        )
    }

    /// Retires every pocket this layer did not reach: the stack has closed over it, and
    /// one that never found a way out is trapped resin.
    fn close_unseen(&mut self, seen: &[bool]) {
        for index in 0..self.pockets.len() {
            if seen.get(index).copied().unwrap_or(false) || self.pockets[index].is_none() {
                continue;
            }
            let pocket = self.pockets[index].take().unwrap_or_else(|| unreachable!());
            self.free.push(index);

            let cell_mm2 = self.grid.millimetres_of(1).powi(2);
            let volume_mm3 = pocket.cells as Scalar * cell_mm2 * self.layer_height_mm;
            if pocket.open || volume_mm3 < MIN_TRAPPED_MM3 {
                continue;
            }
            let middle = self.grid.centre_mm(
                i32::midpoint(pocket.floor.x0, pocket.floor.x1),
                pocket.floor.row,
            );
            self.found.push(Trapped {
                at: Vec3::new(middle.x, middle.y, pocket.floor_z),
                volume_mm3,
                bounds: self.box_of(&pocket),
            });
        }
    }
}

impl Air {
    fn new(row: i32, x0: i32, x1: i32) -> Self {
        Self {
            row,
            x0,
            x1,
            pocket: usize::MAX,
        }
    }
}

/// Where one row's runs sit in the layer's own list.
#[derive(Debug, Clone, Copy)]
struct Row {
    at: i32,
    start: usize,
    end: usize,
}

fn resolve(alias: &FastMap<usize, usize>, mut pocket: usize) -> usize {
    while let Some(&next) = alias.get(&pocket) {
        pocket = next;
    }
    pocket
}

/// Every run of this layer standing over a run of the layer below, with the pocket that
/// one belongs to. Both sides are sorted, so each row is one walk down two lists.
fn links(air: &[Air], rows: &[Row], previous: &[Air]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut below = 0;
    for row in rows {
        while below < previous.len() && previous[below].row < row.at {
            below += 1;
        }
        let mut under = below;
        let mut over = row.start;
        while over < row.end && under < previous.len() && previous[under].row == row.at {
            if air[over].overlaps(previous[under]) {
                pairs.push((over, previous[under].pocket));
            }
            if air[over].x1 < previous[under].x1 {
                over += 1;
            } else {
                under += 1;
            }
        }
    }
    pairs
}

/// The runs of one layer, joined where they touch across neighbouring rows: a union-find
/// over one layer alone, so it is as big as that layer and no bigger.
struct Runs {
    parent: Vec<usize>,
}

impl Runs {
    fn of(air: &[Air], rows: &[Row]) -> Self {
        let mut runs = Self {
            parent: (0..air.len()).collect(),
        };
        for pair in rows.windows(2) {
            let (under, over) = (pair[0], pair[1]);
            if over.at != under.at + 1 {
                continue;
            }
            let (mut a, mut b) = (over.start, under.start);
            while a < over.end && b < under.end {
                if air[a].overlaps(air[b]) {
                    runs.union(a, b);
                }
                if air[a].x1 < air[b].x1 {
                    a += 1;
                } else {
                    b += 1;
                }
            }
        }
        runs
    }

    fn find(&mut self, mut index: usize) -> usize {
        while self.parent[index] != index {
            self.parent[index] = self.parent[self.parent[index]];
            index = self.parent[index];
        }
        index
    }

    fn union(&mut self, left: usize, right: usize) {
        let (left, right) = (self.find(left), self.find(right));
        if left != right {
            self.parent[right] = left;
        }
    }
}

#[cfg(test)]
mod tests;
