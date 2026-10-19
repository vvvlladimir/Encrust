use std::collections::VecDeque;

use core_geometry::{Cut, FastMap, Plane, Scalar, Vec2, Vec3};
use rayon::prelude::*;

use crate::distance;

/// The finest and coarsest cell, mm. A tenth of a millimetre is past what a pixel of
/// most panels resolves; a quarter still finds room for a 2 mm peg.
const FINEST_MM: Scalar = 0.05;
const COARSEST_MM: Scalar = 0.25;

/// Cells across the face's longest side, at most, before the cell is made coarser: a
/// million cells, some twenty megabytes for the grid and its fields.
const MOST_CELLS: Scalar = 1024.0;

/// Cells of empty margin round the face, so every edge of it lies between two cells.
const MARGIN: usize = 2;

/// Marks a cell outside every piece.
const OUTSIDE: u32 = u32::MAX;

/// The plane's own axes, `u × v` along its normal, and where they start.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub normal: Vec3,
}

impl Frame {
    pub fn of(plane: Plane) -> Self {
        let (u, v) = plane.basis();
        Self {
            origin: plane.point,
            u,
            v,
            normal: plane.normal,
        }
    }

    pub fn flat(&self, point: Vec3) -> Vec2 {
        let along = point - self.origin;
        Vec2::new(along.dot(self.u), along.dot(self.v))
    }

    pub fn lift(&self, point: Vec2) -> Vec3 {
        self.origin + self.u * point.x + self.v * point.y
    }
}

/// The face a cut closed a half with, drawn on a square grid in the cut plane: how far
/// each cell's centre is from the face's edge, and which separate piece of it it is in.
#[derive(Debug)]
pub(crate) struct Section {
    pub frame: Frame,
    /// Centre of cell (0, 0), in the frame.
    pub corner: Vec2,
    pub cell_mm: Scalar,
    pub width: usize,
    pub height: usize,
    /// Signed distance from each cell's centre to the edge, mm, positive inside the face.
    pub distance: Vec<Scalar>,
    /// The piece each cell is in, or `OUTSIDE`.
    piece: Vec<u32>,
    pub pieces: usize,
}

impl Section {
    /// The cap `cut` closed its lower half with on `plane`, drawn no coarser than
    /// `coarsest_mm`. `None` where the cut left no cap.
    pub fn of(cut: &Cut, plane: Plane, coarsest_mm: Scalar) -> Option<Self> {
        let frame = Frame::of(plane);
        let edges = edges(cut, &frame);
        let (low, high) = bounds(&edges)?;
        let extent = (high - low).max_element();
        let cell_mm = FINEST_MM
            .min(coarsest_mm.min(COARSEST_MM))
            .max(extent / MOST_CELLS);
        let margin = cell_mm * MARGIN as Scalar;
        let corner = low - Vec2::splat(margin);
        let width = ((high.x - low.x + 2.0 * margin) / cell_mm).ceil() as usize + 1;
        let height = ((high.y - low.y + 2.0 * margin) / cell_mm).ceil() as usize + 1;
        let grid = Grid {
            corner,
            cell_mm,
            width,
            height,
        };
        let inside = grid.fill(&edges);
        let distance = grid.signed(&inside);
        let (piece, pieces) = grid.pieces(&inside);
        Some(Self {
            frame,
            corner,
            cell_mm,
            width,
            height,
            distance,
            piece,
            pieces,
        })
    }

    /// The centre of cell `index`, in the frame.
    pub fn point(&self, index: usize) -> Vec2 {
        let (x, y) = (index % self.width, index / self.width);
        self.corner + Vec2::new(x as Scalar, y as Scalar) * self.cell_mm
    }

    /// The cells of each piece, piece by piece.
    pub fn cells_by_piece(&self) -> Vec<Vec<usize>> {
        let mut cells = vec![Vec::new(); self.pieces];
        for (index, piece) in self.piece.iter().enumerate() {
            if *piece != OUTSIDE {
                cells[*piece as usize].push(index);
            }
        }
        cells
    }

    /// Signed distance from `point` to the edge, mm, read off the nearest cell, and
    /// negative off the grid.
    pub fn distance_at(&self, point: Vec2) -> Scalar {
        let cell = ((point - self.corner) / self.cell_mm).round();
        if cell.x < 0.0 || cell.y < 0.0 {
            return -Scalar::MAX;
        }
        let (x, y) = (cell.x as usize, cell.y as usize);
        if x >= self.width || y >= self.height {
            return -Scalar::MAX;
        }
        self.distance[y * self.width + x]
    }
}

/// The grid a section is drawn on, before it is filled.
struct Grid {
    corner: Vec2,
    cell_mm: Scalar,
    width: usize,
    height: usize,
}

impl Grid {
    /// Which cell centres the closed loops of `edges` hold, by the even-odd rule.
    fn fill(&self, edges: &[(Vec2, Vec2)]) -> Vec<bool> {
        let mut crossings = vec![Vec::new(); self.height];
        for (a, b) in edges {
            let (low, high) = (a.y.min(b.y), a.y.max(b.y));
            let first = ((low - self.corner.y) / self.cell_mm).floor().max(0.0) as usize;
            let last = (((high - self.corner.y) / self.cell_mm).ceil() as usize).min(self.height);
            for (row, found) in crossings.iter_mut().enumerate().take(last).skip(first) {
                let y = self.corner.y + row as Scalar * self.cell_mm;
                if (a.y > y) != (b.y > y) {
                    found.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        let mut inside = vec![false; self.width * self.height];
        inside
            .par_chunks_mut(self.width)
            .zip(crossings.par_iter_mut())
            .for_each(|(row, found)| {
                found.sort_by(Scalar::total_cmp);
                for [enter, leave] in found.as_chunks::<2>().0 {
                    let from = self.column_at_or_after(*enter);
                    let to = self.column_at_or_after(*leave);
                    row[from..to].fill(true);
                }
            });
        inside
    }

    fn column_at_or_after(&self, x: Scalar) -> usize {
        (((x - self.corner.x) / self.cell_mm).ceil().max(0.0) as usize).min(self.width)
    }

    /// Distance to the edge at every cell centre, mm, positive inside. A centre is half a
    /// cell short of the edge beyond its nearest neighbour across it.
    fn signed(&self, inside: &[bool]) -> Vec<Scalar> {
        let outside: Vec<bool> = inside.iter().map(|cell| !cell).collect();
        let to_outside = distance::squared(self.width, self.height, &outside);
        let to_inside = distance::squared(self.width, self.height, inside);
        let half = 0.5;
        inside
            .par_iter()
            .zip(to_outside.par_iter().zip(to_inside.par_iter()))
            .map(|(is_inside, (out, into))| {
                if *is_inside {
                    (out.sqrt() - half) * self.cell_mm
                } else {
                    -(into.sqrt() - half) * self.cell_mm
                }
            })
            .collect()
    }

    /// The four-connected pieces of `inside`, and how many there are.
    fn pieces(&self, inside: &[bool]) -> (Vec<u32>, usize) {
        let mut piece = vec![OUTSIDE; inside.len()];
        let mut count = 0u32;
        let mut queue = VecDeque::new();
        for start in 0..inside.len() {
            if !inside[start] || piece[start] != OUTSIDE {
                continue;
            }
            piece[start] = count;
            queue.push_back(start);
            while let Some(cell) = queue.pop_front() {
                let (x, y) = (cell % self.width, cell / self.width);
                let neighbours = [
                    (x > 0).then(|| cell - 1),
                    (x + 1 < self.width).then(|| cell + 1),
                    (y > 0).then(|| cell - self.width),
                    (y + 1 < self.height).then(|| cell + self.width),
                ];
                for next in neighbours.into_iter().flatten() {
                    if inside[next] && piece[next] == OUTSIDE {
                        piece[next] = count;
                        queue.push_back(next);
                    }
                }
            }
            count += 1;
        }
        (piece, count as usize)
    }
}

/// The edges round the cap `cut` closed its lower half with, flattened into `frame`: those
/// only one of the cap's triangles has.
fn edges(cut: &Cut, frame: &Frame) -> Vec<(Vec2, Vec2)> {
    let half = &cut.below;
    let cap = &half.faces[half.faces.len().saturating_sub(cut.cap_faces)..];
    let mut uses: FastMap<(u32, u32), usize> = FastMap::default();
    for face in cap {
        for side in 0..3 {
            let (a, b) = (face[side], face[(side + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    uses.into_iter()
        .filter(|(_, count)| *count == 1)
        .map(|((a, b), _)| {
            (
                frame.flat(half.vertices[a as usize]),
                frame.flat(half.vertices[b as usize]),
            )
        })
        .collect()
}

fn bounds(edges: &[(Vec2, Vec2)]) -> Option<(Vec2, Vec2)> {
    let points = edges.iter().flat_map(|(a, b)| [*a, *b]);
    let low = points.clone().reduce(Vec2::min)?;
    let high = points.reduce(Vec2::max)?;
    Some((low, high))
}

#[cfg(test)]
mod tests {
    use core_geometry::{cuboid, cut};

    use super::*;

    fn lower_face(min: Vec3, max: Vec3, height_mm: Scalar) -> Section {
        let plane = Plane::at(Vec3::Z, height_mm).expect("+Z is a direction");
        Section::of(&cut(&cuboid(min, max), plane), plane, COARSEST_MM)
            .expect("the cut crosses the box")
    }

    #[test]
    fn the_middle_of_a_square_face_is_half_its_side_from_the_edge() {
        let section = lower_face(Vec3::ZERO, Vec3::splat(20.0), 10.0);
        let middle = section.frame.flat(Vec3::new(10.0, 10.0, 10.0));
        let distance = section.distance_at(middle);
        assert!(
            (distance - 10.0).abs() <= section.cell_mm,
            "a 20 mm square is 10 mm from its edge at the middle, got {distance}"
        );
        assert_eq!(section.pieces, 1);
    }

    #[test]
    fn two_boxes_cut_together_are_two_pieces() {
        let mut both = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        both.append(&cuboid(
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(30.0, 10.0, 10.0),
        ));
        let plane = Plane::at(Vec3::Z, 5.0).expect("+Z is a direction");
        let section =
            Section::of(&cut(&both, plane), plane, COARSEST_MM).expect("the cut crosses both");
        assert_eq!(section.pieces, 2);
        let area: Scalar = section
            .cells_by_piece()
            .iter()
            .map(|cells| cells.len() as Scalar * section.cell_mm * section.cell_mm)
            .sum();
        assert!(
            (area - 200.0).abs() < 10.0,
            "two 10 mm squares cover 200 mm2, got {area}"
        );
    }
}
