use core_geometry::{FastMap, Scalar, Vec2, triangulate};

use crate::outline::Shape;

/// The most rounds of splitting a fill takes, whatever is left long after them: each round
/// halves every edge still too long, so twenty take a metre down to a micrometre.
const MOST_ROUNDS: usize = 20;

/// A shape filled flat: its points, the outline's first, and the triangles of the fill,
/// facing up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sheet {
    pub flat: Vec<Vec2>,
    pub cap: Vec<[u32; 3]>,
}

impl Sheet {
    /// `shape` filled with as few triangles as its outline allows.
    pub fn new(shape: &Shape) -> Self {
        let mut flat: Vec<Vec2> = shape.outer.clone();
        let mut rings: Vec<Vec<u32>> = vec![(0..shape.outer.len() as u32).collect()];
        for hole in &shape.holes {
            let start = flat.len() as u32;
            flat.extend_from_slice(hole);
            rings.push((start..flat.len() as u32).collect());
        }
        let holes: Vec<&Vec<u32>> = rings[1..].iter().collect();
        // The outer ring runs anticlockwise, so the fill, wound as it is, faces up.
        let cap = triangulate(&rings[0], &holes, &flat);
        Self { flat, cap }
    }

    /// `shape` filled with triangles no edge of which is longer than `spacing_mm`, so that
    /// laid on a surface point by point it bends with it.
    pub fn fine(shape: &Shape, spacing_mm: Scalar) -> Self {
        let mut sheet = Self::new(shape);
        sheet.refine(spacing_mm);
        sheet
    }

    /// Splits every edge of the fill longer than `spacing_mm` at its middle, round after
    /// round, each triangle into the two, three or four its split edges leave.
    ///
    /// An edge is split once for both triangles on it, so the letter stays closed.
    fn refine(&mut self, spacing_mm: Scalar) {
        let limit = spacing_mm * spacing_mm;
        let mut middles: FastMap<(u32, u32), u32> = FastMap::default();
        for _ in 0..MOST_ROUNDS {
            let before = middles.len();
            for triangle in &self.cap {
                for (a, b) in edges(*triangle) {
                    let (from, to) = (self.flat[a as usize], self.flat[b as usize]);
                    if from.distance_squared(to) <= limit {
                        continue;
                    }
                    middles.entry((a.min(b), a.max(b))).or_insert_with(|| {
                        self.flat.push((from + to) / 2.0);
                        self.flat.len() as u32 - 1
                    });
                }
            }
            if middles.len() == before {
                break;
            }
            let mut finer = Vec::with_capacity(self.cap.len() * 3);
            for triangle in &self.cap {
                split(*triangle, &middles, &mut finer);
            }
            self.cap = finer;
        }
    }
}

fn edges([a, b, c]: [u32; 3]) -> [(u32, u32); 3] {
    [(a, b), (b, c), (c, a)]
}

/// `triangle` cut at the middles of its edges found in `middles`, wound as it was.
fn split(triangle: [u32; 3], middles: &FastMap<(u32, u32), u32>, out: &mut Vec<[u32; 3]>) {
    let middle = |(a, b): (u32, u32)| middles.get(&(a.min(b), a.max(b))).copied();
    let cut = edges(triangle).map(middle);
    let v = triangle;
    match cut {
        [None, None, None] => out.push(triangle),
        [Some(ab), Some(bc), Some(ca)] => {
            out.extend([[v[0], ab, ca], [ab, v[1], bc], [ca, bc, v[2]], [ab, bc, ca]]);
        }
        _ => {
            let whole = cut.iter().filter(|middle| middle.is_none()).count();
            // Turned so the edge from `a` to `b` is split, and with two split, `c` to `a` is not.
            let turn = (0..3)
                .find(|&i| match whole {
                    2 => cut[i].is_some(),
                    _ => cut[(i + 2) % 3].is_none(),
                })
                .unwrap_or(0);
            let [a, b, c] = [v[turn], v[(turn + 1) % 3], v[(turn + 2) % 3]];
            let ab = cut[turn].unwrap_or(a);
            match cut[(turn + 1) % 3] {
                Some(bc) if whole == 1 => out.extend([[ab, b, bc], [a, ab, bc], [a, bc, c]]),
                _ => out.extend([[a, ab, c], [ab, b, c]]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: Scalar) -> Shape {
        Shape {
            outer: vec![
                Vec2::ZERO,
                Vec2::new(side, 0.0),
                Vec2::new(side, side),
                Vec2::new(0.0, side),
            ],
            holes: Vec::new(),
        }
    }

    fn area(sheet: &Sheet) -> Scalar {
        sheet
            .cap
            .iter()
            .map(|[a, b, c]| {
                let (a, b, c) = (
                    sheet.flat[*a as usize],
                    sheet.flat[*b as usize],
                    sheet.flat[*c as usize],
                );
                (b - a).perp_dot(c - a) / 2.0
            })
            .sum()
    }

    #[test]
    fn a_fine_fill_keeps_every_edge_within_the_spacing_and_covers_the_shape_once() {
        let sheet = Sheet::fine(&square(4.0), 0.5);
        let longest = sheet
            .cap
            .iter()
            .flat_map(|triangle| edges(*triangle))
            .map(|(a, b)| sheet.flat[a as usize].distance(sheet.flat[b as usize]))
            .fold(0.0, Scalar::max);
        assert!(longest <= 0.5 + 1e-5, "no edge past 0.5 mm; got {longest}");
        let area = area(&sheet);
        assert!(
            (area - 16.0).abs() < 1e-3,
            "a 4 mm square, every triangle facing up and none twice; got {area} mm²"
        );
        assert!(sheet.cap.iter().all(|[a, b, c]| {
            let (a, b, c) = (
                sheet.flat[*a as usize],
                sheet.flat[*b as usize],
                sheet.flat[*c as usize],
            );
            (b - a).perp_dot(c - a) > 0.0
        }));
    }
}
