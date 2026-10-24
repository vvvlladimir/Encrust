use core_geometry::{Buckets, Scalar, Vec2, Vec3};
use core_slicer::{Layer, Sliced};
use rayon::prelude::*;

use crate::config::REFERENCE_RISE_MM;
use crate::field::{Field, Grid};

/// Cell size islands are read at, millimetres: finer than any printer's pixel, so a piece
/// the printed stack sees standing on nothing is seen here too.
const FINE_PITCH_MM: Scalar = 0.02;

/// The least piece standing on nothing that is an island, mm2: half the stack's own check, whose
/// pixels fall apart from these cells; smaller reads as not there.
const DUST_MM2: Scalar = 0.005;

/// How far a contact is moved onto an island, across the plate and down, or along its face to room
/// for a head, mm: under a pillar width, so what it held is still held (ADR 0330, 0348).
pub(crate) const MOVE_REACH_MM: Scalar = 0.3;

/// How far inside an island a contact stands to hold it, mm: half the thinnest tip's radius; an
/// island too thin for that is held by a contact anywhere on it.
const SEAT_MM: Scalar = 0.06;

/// Side of a bucket contacts and low points are kept in, millimetres.
const BUCKET_MM: Scalar = 1.0;

/// How many layers are read onto the fine grid in one go, as placement reads its own.
const READ_BLOCK: usize = 32;

/// Holds every island `stack` begins that no contact touches: moves the nearest of `placed` onto
/// it or returns a new contact and the island's area in mm2 (ADR 0330); `skip` bars places.
pub(crate) fn hold_islands(
    stack: &Sliced,
    layer_height_mm: Scalar,
    placed: &mut [Vec3],
    seeds: &[Vec3],
    lows: &[Vec3],
    skip: impl Fn(Vec3) -> bool,
) -> Vec<(Vec3, Scalar)> {
    let Some(grid) = grid_of(stack, FINE_PITCH_MM) else {
        return Vec::new();
    };
    let mut contacts = Contacts::new(placed, seeds);
    let lows = Points::new(lows.to_vec());
    let cell_mm2 = FINE_PITCH_MM * FINE_PITCH_MM;
    let mut found = Vec::new();
    // The layer under the block, and the specks on it too small to name, which nothing
    // stands on: the printed stack may never have cured them at all.
    let (mut under, mut dust) = (Field::default(), Field::default());
    for layers in stack.layers.chunks(READ_BLOCK) {
        let fields: Vec<Field> = layers
            .par_iter()
            .map(|layer| Field::of_layer(layer, &grid))
            .collect();
        // Read against the whole layer under each; only a speck under one asks again.
        let mut pieces: Vec<Vec<Field>> = (0..fields.len())
            .into_par_iter()
            .map(|at| {
                standing_on_nothing(
                    &fields[at],
                    fields.get(at.wrapping_sub(1)).unwrap_or(&under),
                )
            })
            .collect();
        for (at, layer) in layers.iter().enumerate() {
            if !dust.is_empty() {
                let below = fields.get(at.wrapping_sub(1)).unwrap_or(&under);
                pieces[at] = standing_on_nothing(&fields[at], &below.without(&dust));
            }
            dust = Field::default();
            for piece in &pieces[at] {
                let area_mm2 = piece.cell_count() as Scalar * cell_mm2;
                if area_mm2 < DUST_MM2 {
                    dust = dust.with(piece);
                    continue;
                }
                let island = Island::new(piece, &grid, layer, layer_height_mm);
                if let Some(touching) = island.touched_by(&contacts) {
                    contacts.pinned[touching] = true;
                    continue;
                }
                let at = island.start(&lows);
                if skip(at) {
                    continue;
                }
                match island.movable(&contacts, at) {
                    Some(nearest) => contacts.move_to(nearest, at),
                    None => found.push((at, area_mm2)),
                }
            }
        }
        under = fields.into_iter().last().unwrap_or_default();
    }
    placed.copy_from_slice(&contacts.points.points[..placed.len()]);
    found
}

/// The pieces of `field` that touch nothing of `below`, corner to corner included.
fn standing_on_nothing(field: &Field, below: &Field) -> Vec<Field> {
    let fresh = field.without(below);
    let carried = field.without(&fresh);
    // Most of what a layer adds is a sliver along a wall, every cell of it beside the wall.
    if fresh.uncarried_by(&carried, 1).is_empty() {
        return Vec::new();
    }
    fresh
        .pieces()
        .into_iter()
        .filter(|piece| !piece.adjoins(&carried))
        .collect()
}

/// One piece of a layer standing on nothing, read on the fine grid.
struct Island<'a> {
    piece: &'a Field,
    /// What of it lies [`SEAT_MM`] in from its rim, or all of it where nothing does.
    seat: Field,
    grid: &'a Grid,
    layer: &'a Layer,
    layer_height_mm: Scalar,
}

impl<'a> Island<'a> {
    fn new(piece: &'a Field, grid: &'a Grid, layer: &'a Layer, layer_height_mm: Scalar) -> Self {
        let seat = piece.shrunk(grid.cells_of(SEAT_MM));
        Self {
            seat: if seat.is_empty() { piece.clone() } else { seat },
            piece,
            grid,
            layer,
            layer_height_mm,
        }
    }

    /// Whether `z` is where a column holds the piece from the layer below: no higher than
    /// its plane and no more than two layers under it.
    fn holds_at(&self, z: Scalar) -> bool {
        let bottom_z = self.layer.z - 2.0 * self.layer_height_mm;
        (bottom_z - 1e-4..=self.layer.z + 1e-4).contains(&z)
    }

    /// The contact standing on the piece's seat where a column holds it, if one does.
    fn touched_by(&self, contacts: &Contacts) -> Option<usize> {
        contacts.points.near(self.bounds()).find(|index| {
            let contact = contacts.points.points[*index];
            let [x, y] = self.grid.cell_of(contact.truncate());
            self.holds_at(contact.z) && self.seat.contains(x, y)
        })
    }

    /// The placed contact to move to `at` that no island has claimed: one on the piece's rim where
    /// a column holds it, else the nearest within reach across the plate and of the piece's height.
    fn movable(&self, contacts: &Contacts, at: Vec3) -> Option<usize> {
        let free = |index: &usize| *index < contacts.placed && !contacts.pinned[*index];
        let on_rim = contacts
            .points
            .near(self.bounds())
            .filter(free)
            .find(|index| {
                let contact = contacts.points.points[*index];
                let [x, y] = self.grid.cell_of(contact.truncate());
                self.holds_at(contact.z) && self.piece.contains(x, y)
            });
        let reach = Vec2::splat(MOVE_REACH_MM);
        let distance = |index: &usize| {
            contacts.points.points[*index]
                .truncate()
                .distance(at.truncate())
        };
        on_rim.or_else(|| {
            contacts
                .points
                .near((at.truncate() - reach, at.truncate() + reach))
                .filter(free)
                .filter(|index| {
                    let z = contacts.points.points[*index].z;
                    self.holds_at(z) || (self.layer.z..=self.layer.z + MOVE_REACH_MM).contains(&z)
                })
                .filter(|index| distance(index) <= MOVE_REACH_MM)
                .min_by(|a, b| distance(a).total_cmp(&distance(b)).then(a.cmp(b)))
        })
    }

    /// Where its support goes: the lowest model point on its seat, else the seat cell nearest its
    /// middle, a layer under the plane as every placed contact is.
    fn start(&self, lows: &Points) -> Vec3 {
        let floor_z = self.layer.z - REFERENCE_RISE_MM;
        let lowest = lows
            .near(self.bounds())
            .map(|index| lows.points[index])
            .filter(|low| (floor_z..=self.layer.z).contains(&low.z))
            .filter(|low| {
                let [x, y] = self.grid.cell_of(low.truncate());
                self.seat.contains(x, y)
            })
            .min_by(|a, b| a.z.total_cmp(&b.z));
        lowest.unwrap_or_else(|| self.middle().extend(self.layer.z - self.layer_height_mm))
    }

    /// The cell of the seat nearest the mean of its cells.
    fn middle(&self) -> Vec2 {
        let cells = self.cells();
        let mean = cells.iter().copied().sum::<Vec2>() / cells.len().max(1) as Scalar;
        cells
            .into_iter()
            .min_by(|a, b| {
                a.distance_squared(mean)
                    .total_cmp(&b.distance_squared(mean))
            })
            .unwrap_or(mean)
    }

    /// The middles of the seat's cells on the plate.
    fn cells(&self) -> Vec<Vec2> {
        self.seat
            .rows()
            .flat_map(|(y, spans)| {
                spans
                    .iter()
                    .flat_map(move |span| (span.x0..span.x1).map(move |x| (x, y)))
            })
            .map(|(x, y)| self.grid.centre_mm(x, y))
            .collect()
    }

    /// The middles of the piece's corner cells on the plate.
    fn bounds(&self) -> (Vec2, Vec2) {
        let (min, max) = self.piece.bounds().unwrap_or_default();
        (
            self.grid.centre_mm(min[0], min[1]),
            self.grid.centre_mm(max[0], max[1]),
        )
    }
}

/// The placed contacts, which may be moved, then the seeds, which may not, and which of
/// them an island has claimed.
struct Contacts {
    points: Points,
    placed: usize,
    pinned: Vec<bool>,
}

impl Contacts {
    fn new(placed: &[Vec3], seeds: &[Vec3]) -> Self {
        let points: Vec<Vec3> = placed.iter().chain(seeds).copied().collect();
        Self {
            pinned: vec![false; points.len()],
            points: Points::new(points),
            placed: placed.len(),
        }
    }

    /// Moves placed contact `index` to `at`, where it now holds an island.
    fn move_to(&mut self, index: usize, at: Vec3) {
        let from = self.points.points[index].truncate();
        self.points.buckets.retain_at(from, |other| *other != index);
        self.points.points[index] = at;
        self.points.buckets.insert(at.truncate(), index);
        self.pinned[index] = true;
    }
}

/// Points, and their indices in buckets of the plate [`BUCKET_MM`] across.
struct Points {
    points: Vec<Vec3>,
    buckets: Buckets<usize>,
}

impl Points {
    fn new(points: Vec<Vec3>) -> Self {
        let mut buckets = Buckets::new(BUCKET_MM);
        for (index, point) in points.iter().enumerate() {
            buckets.insert(point.truncate(), index);
        }
        Self { points, buckets }
    }

    /// The indices of the points whose bucket meets the box `min`..`max`.
    fn near(&self, (min, max): (Vec2, Vec2)) -> impl Iterator<Item = usize> + '_ {
        self.buckets.within(min, max).copied()
    }
}

/// A grid of `pitch_mm` cells under the whole stack, or `None` when there is nothing on it.
pub(crate) fn grid_of(stack: &Sliced, pitch_mm: Scalar) -> Option<Grid> {
    let mut min = Vec2::splat(Scalar::INFINITY);
    let mut max = Vec2::splat(Scalar::NEG_INFINITY);
    for point in stack
        .layers
        .iter()
        .flat_map(|layer| &layer.contours)
        .flat_map(|contour| &contour.points)
    {
        min = min.min(*point);
        max = max.max(*point);
    }
    (min.x <= max.x).then(|| Grid::covering(min, max, pitch_mm))
}
