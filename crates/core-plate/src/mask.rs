use core_geometry::{Mesh, Scalar, Transform, Vec2};

/// Cells of a mask, one bit each, packed into 64-bit words a row at a time.
///
/// A row is words rather than bytes so that testing a part against the plate is an `and`
/// of a few words a row instead of a loop over cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    words: Vec<u64>,
    width: usize,
    height: usize,
    /// Words in one row.
    stride: usize,
}

const BITS: usize = 64;

impl Mask {
    pub fn empty(width: usize, height: usize) -> Self {
        let stride = width.div_ceil(BITS);
        Self {
            words: vec![0; stride * height],
            width,
            height,
            stride,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    pub fn count(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.words[y * self.stride + x / BITS] |= 1 << (x % BITS);
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.words[y * self.stride + x / BITS] & (1 << (x % BITS)) != 0
    }

    /// Whether this mask, laid down with its corner at `(at_x, at_y)`, touches anything
    /// already set in `other`.
    pub fn hits(&self, other: &Mask, at_x: usize, at_y: usize) -> bool {
        if at_x + self.width > other.width || at_y + self.height > other.height {
            return true;
        }
        let mut row = Vec::with_capacity(other.stride);
        for y in 0..self.height {
            self.shifted_row(y, at_x, other.stride, &mut row);
            let target = &other.words[(at_y + y) * other.stride..][..other.stride];
            if row.iter().zip(target).any(|(a, b)| a & b != 0) {
                return true;
            }
        }
        false
    }

    /// Ors this mask into `other` with its corner at `(at_x, at_y)`.
    pub fn paint(&self, other: &mut Mask, at_x: usize, at_y: usize) {
        let mut row = Vec::with_capacity(other.stride);
        for y in 0..self.height {
            if at_y + y >= other.height {
                break;
            }
            self.shifted_row(y, at_x, other.stride, &mut row);
            let target = &mut other.words[(at_y + y) * other.stride..][..other.stride];
            for (word, shifted) in target.iter_mut().zip(&row) {
                *word |= shifted;
            }
        }
    }

    /// The mask grown by `cells` in every direction, which is how a clearance is kept:
    /// what a part must not touch is its neighbours plus the gap around them.
    ///
    /// The result is `cells` bigger on each side, so the gap reaches outside the part
    /// rather than eating into it. Its corner sits `cells` before the original's.
    pub fn grown(&self, cells: usize) -> Mask {
        if cells == 0 {
            return self.clone();
        }
        let mut grown = Mask::empty(self.width + 2 * cells, self.height + 2 * cells);
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.get(x, y) {
                    continue;
                }
                for dy in y..=(y + 2 * cells) {
                    for dx in x..=(x + 2 * cells) {
                        grown.set(dx, dy);
                    }
                }
            }
        }
        grown
    }

    /// One row of this mask shifted right by `at_x` bits, written into `out` as `stride`
    /// words. A row crosses word boundaries, so the shift carries between words.
    fn shifted_row(&self, y: usize, at_x: usize, stride: usize, out: &mut Vec<u64>) {
        out.clear();
        out.resize(stride, 0);

        let word_shift = at_x / BITS;
        let bit_shift = at_x % BITS;
        for word in 0..self.stride {
            let source = self.words[y * self.stride + word];
            if source == 0 {
                continue;
            }
            if let Some(slot) = out.get_mut(word + word_shift) {
                *slot |= source << bit_shift;
            }
            // The part of the word pushed past its end lands in the next one. Shifting a
            // u64 by 64 is undefined in Rust, so a zero shift is the case to skip.
            if bit_shift > 0
                && let Some(slot) = out.get_mut(word + word_shift + 1)
            {
                *slot |= source >> (BITS - bit_shift);
            }
        }
    }
}

/// The shadow a model casts on the plate, in cells of `cell_mm`, with the corner the
/// cells start at.
#[derive(Debug, Clone, PartialEq)]
pub struct Footprint {
    pub mask: Mask,
    /// Where cell (0, 0) starts, in plate millimetres.
    pub corner_mm: Vec2,
    pub cell_mm: Scalar,
}

impl Footprint {
    /// Rasterises where a placed model stands on the plate.
    ///
    /// Every triangle is projected and filled, and its edges are walked as well, so a
    /// sliver thinner than a cell still marks the cells it crosses: the mask has to cover
    /// the model, never merely sample it.
    pub fn of(mesh: &Mesh, transform: Transform, cell_mm: Scalar) -> Option<Self> {
        if cell_mm <= 0.0 {
            return None;
        }
        let matrix = transform.to_matrix();
        let placed: Vec<Vec2> = mesh
            .vertices
            .iter()
            .map(|vertex| matrix.transform_point3(*vertex).truncate())
            .collect();
        if placed.is_empty() || mesh.faces.is_empty() {
            return None;
        }

        let (mins, maxs) = placed.iter().fold(
            (
                Vec2::splat(Scalar::INFINITY),
                Vec2::splat(Scalar::NEG_INFINITY),
            ),
            |(lo, hi), point| (lo.min(*point), hi.max(*point)),
        );
        let size = maxs - mins;
        let mut mask = Mask::empty(
            ((size.x / cell_mm).ceil() as usize).max(1),
            ((size.y / cell_mm).ceil() as usize).max(1),
        );

        for face in &mesh.faces {
            let corners = [
                placed[face[0] as usize] - mins,
                placed[face[1] as usize] - mins,
                placed[face[2] as usize] - mins,
            ];
            fill(&mut mask, corners, cell_mm);
        }

        Some(Self {
            mask,
            corner_mm: mins,
            cell_mm,
        })
    }

    pub fn area_cells(&self) -> usize {
        self.mask.count()
    }
}

/// Marks every cell a triangle covers, and every cell its edges cross.
fn fill(mask: &mut Mask, corners: [Vec2; 3], cell_mm: Scalar) {
    let lo = corners[0].min(corners[1]).min(corners[2]) / cell_mm;
    let hi = corners[0].max(corners[1]).max(corners[2]) / cell_mm;

    for y in lo.y.floor().max(0.0) as usize..=(hi.y.floor().max(0.0) as usize) {
        for x in lo.x.floor().max(0.0) as usize..=(hi.x.floor().max(0.0) as usize) {
            let centre = Vec2::new((x as Scalar + 0.5) * cell_mm, (y as Scalar + 0.5) * cell_mm);
            if inside(corners, centre) {
                mask.set(x, y);
            }
        }
    }

    for edge in 0..3 {
        walk(mask, corners[edge], corners[(edge + 1) % 3], cell_mm);
    }
}

fn inside(corners: [Vec2; 3], point: Vec2) -> bool {
    let side = |a: Vec2, b: Vec2| (b - a).perp_dot(point - a);
    let (first, second, third) = (
        side(corners[0], corners[1]),
        side(corners[1], corners[2]),
        side(corners[2], corners[0]),
    );
    (first >= 0.0 && second >= 0.0 && third >= 0.0)
        || (first <= 0.0 && second <= 0.0 && third <= 0.0)
}

/// Marks the cells a segment passes through, sampling it every half cell.
fn walk(mask: &mut Mask, from: Vec2, to: Vec2, cell_mm: Scalar) {
    let steps = ((to - from).length() / (cell_mm / 2.0)).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let point = from.lerp(to, step as Scalar / steps as Scalar) / cell_mm;
        if point.x >= 0.0 && point.y >= 0.0 {
            mask.set(point.x.floor() as usize, point.y.floor() as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_box_covers_every_cell_of_its_own_footprint() {
        let footprint = Footprint::of(
            &cuboid(Vec3::new(10.0, 6.0, 4.0)),
            Transform::default(),
            1.0,
        )
        .expect("a box has a footprint");

        assert_eq!((footprint.mask.width(), footprint.mask.height()), (10, 6));
        assert_eq!(footprint.area_cells(), 60, "the whole 10 by 6 rectangle");
        assert_eq!(footprint.corner_mm, Vec2::ZERO);
    }

    #[test]
    fn a_footprint_follows_the_placement() {
        let placed = Transform::from_translation(Vec3::new(20.0, 5.0, 0.0));
        let footprint =
            Footprint::of(&cuboid(Vec3::splat(4.0)), placed, 1.0).expect("a box has a footprint");
        assert_eq!(footprint.corner_mm, Vec2::new(20.0, 5.0));
    }

    #[test]
    fn a_sliver_thinner_than_a_cell_still_marks_cells() {
        // A triangle 20 mm long and 0.05 mm wide: no cell centre falls inside it.
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.05, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        let footprint =
            Footprint::of(&mesh, Transform::default(), 1.0).expect("a sliver has a footprint");
        assert!(
            footprint.area_cells() >= 20,
            "the edges are walked, got {} cells",
            footprint.area_cells()
        );
    }

    #[test]
    fn growing_a_mask_reaches_out_by_the_cells_asked_for() {
        let mut mask = Mask::empty(1, 1);
        mask.set(0, 0);
        let grown = mask.grown(2);

        // The gap reaches outside the part: a single cell grown by two is a 5 by 5 block,
        // and the corner it is painted at moves back by the same two.
        assert_eq!((grown.width(), grown.height()), (5, 5));
        assert_eq!(grown.count(), 25);
    }

    #[test]
    fn a_mask_hits_what_was_painted_where_it_would_go() {
        let mut plate = Mask::empty(200, 4);
        let mut part = Mask::empty(2, 2);
        part.set(0, 0);
        part.set(1, 1);

        // Painted across a word boundary, to catch a shift that loses the carry.
        part.paint(&mut plate, 63, 0);
        assert!(plate.get(63, 0) && plate.get(64, 1));

        assert!(part.hits(&plate, 63, 0));
        assert!(!part.hits(&plate, 100, 0), "nothing is painted out there");
        assert!(
            part.hits(&plate, 199, 0),
            "a part hanging off the edge does not fit"
        );
    }
}
