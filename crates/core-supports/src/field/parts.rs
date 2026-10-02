use super::{Field, Span};

impl Field {
    /// The connected pieces of this field, spans that touch corner to corner counting as
    /// one piece. Ordered bottom row first, which is what keeps a run repeatable.
    pub fn pieces(&self) -> Vec<Self> {
        let (labels, count) = self.labelled();
        pieces_of(self, &labels, count)
    }

    /// The same pieces, keeping which one each span belongs to.
    pub fn parts(&self) -> Parts {
        let (labels, count) = self.labelled();
        Parts {
            field: self.clone(),
            labels,
            count,
        }
    }

    /// The piece each span belongs to, and how many pieces there are.
    fn labelled(&self) -> (Vec<u32>, usize) {
        if self.is_empty() {
            return (Vec::new(), 0);
        }

        let mut sets = DisjointSets::default();
        let mut labels: Vec<usize> = Vec::with_capacity(self.spans.len());
        let mut previous = 0..0usize;
        for edges in self.starts.windows(2) {
            let row = edges[0] as usize..edges[1] as usize;
            // Both rows run left to right, so the spans under this one are found by
            // walking the row below forwards and never going back.
            let mut under = previous.start;
            for at in row.clone() {
                let span = self.spans[at];
                while under < previous.end && self.spans[under].x1 < span.x0 {
                    under += 1;
                }
                let mut label = sets.make();
                let mut ahead = under;
                while ahead < previous.end && self.spans[ahead].x0 <= span.x1 {
                    label = sets.union(label, labels[ahead]);
                    ahead += 1;
                }
                labels.push(label);
            }
            previous = row;
        }

        // Rows are walked upwards, so a piece is first seen on its own lowest row, which
        // is the order the pieces are numbered in and what keeps a run repeatable.
        let mut number = vec![u32::MAX; labels.len()];
        let mut count = 0u32;
        let numbered = labels
            .into_iter()
            .map(|label| {
                let root = sets.find(label);
                if number[root] == u32::MAX {
                    number[root] = count;
                    count += 1;
                }
                number[root]
            })
            .collect();

        (numbered, count as usize)
    }
}

/// The pieces `labels` cuts `field` into, in the order they are numbered.
fn pieces_of(field: &Field, labels: &[u32], count: usize) -> Vec<Field> {
    let mut pieces: Vec<Vec<(i32, Span)>> = vec![Vec::new(); count];
    let mut at = 0;
    for (y, spans) in field.rows() {
        for span in spans {
            pieces[labels[at] as usize].push((y, *span));
            at += 1;
        }
    }
    pieces.into_iter().map(Field::of_spans).collect()
}

/// The pieces of a field, keeping which piece each of its spans belongs to.
///
/// Placement needs more than the pieces themselves: it has to tell which piece a cell
/// falls in, and to follow a piece onto the pieces of the layer under it, so that a
/// support holds only the part of the model it stands under. See `docs/decisions/0081`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parts {
    field: Field,
    /// One piece number per span of `field`, counted from zero in the order the pieces
    /// are first seen.
    labels: Vec<u32>,
    count: usize,
}

impl Parts {
    /// How many pieces the field has.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Which piece covers a cell, or `None` where nothing does.
    pub fn at(&self, x: i32, y: i32) -> Option<u32> {
        let (spans, labels) = self.row(y);
        spans
            .iter()
            .position(|span| span.x0 <= x && x < span.x1)
            .map(|at| labels[at])
    }

    /// For every piece here, the pieces of `below` it stands on, corner contact included.
    ///
    /// Both fields are run-length and sorted, so each row is walked once from left to
    /// right: a layer's parentage costs its outline, not its area.
    pub fn parents(&self, below: &Self) -> Vec<Vec<u32>> {
        let mut parents = vec![Vec::new(); self.count];
        for (y, spans) in self.field.rows() {
            let (_, labels) = self.row(y);
            let (under, under_labels) = below.row(y);
            let mut first = 0;
            for (span, label) in spans.iter().zip(labels) {
                while first < under.len() && under[first].x1 < span.x0 {
                    first += 1;
                }
                let mut ahead = first;
                while ahead < under.len() && under[ahead].x0 <= span.x1 {
                    let parent = under_labels[ahead];
                    let found: &mut Vec<u32> = &mut parents[*label as usize];
                    if !found.contains(&parent) {
                        found.push(parent);
                    }
                    ahead += 1;
                }
            }
        }
        parents
    }

    /// One row's spans and the piece each of them belongs to.
    fn row(&self, y: i32) -> (&[Span], &[u32]) {
        let Ok(index) = usize::try_from(y - self.field.y0) else {
            return (&[], &[]);
        };
        match (
            self.field.starts.get(index),
            self.field.starts.get(index + 1),
        ) {
            (Some(&from), Some(&to)) => (
                &self.field.spans[from as usize..to as usize],
                &self.labels[from as usize..to as usize],
            ),
            _ => (&[], &[]),
        }
    }
}

/// Union-find over span labels, for collecting the pieces of a field.
#[derive(Debug, Default)]
struct DisjointSets {
    parent: Vec<usize>,
}

impl DisjointSets {
    fn make(&mut self) -> usize {
        self.parent.push(self.parent.len());
        self.parent.len() - 1
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent[node] != node {
            self.parent[node] = self.parent[self.parent[node]];
            node = self.parent[node];
        }
        node
    }

    fn union(&mut self, a: usize, b: usize) -> usize {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[b] = a;
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec2;

    use super::*;
    use crate::field::tests::{grid, rectangle, square};

    #[test]
    fn separate_squares_are_separate_pieces_and_touching_ones_are_one() {
        let apart = square(10.0, 10.0, 5.0).with(&square(25.0, 25.0, 5.0));
        assert_eq!(apart.pieces().len(), 2);

        let joined = square(10.0, 10.0, 5.0).with(&square(14.0, 10.0, 5.0));
        assert_eq!(joined.pieces().len(), 1, "they overlap, so they are one");
    }

    #[test]
    fn a_piece_keeps_the_cells_it_was_cut_from() {
        let field = square(10.0, 10.0, 5.0).with(&square(25.0, 25.0, 3.0));
        let pieces = field.pieces();
        let total: u64 = pieces.iter().map(Field::cell_count).sum();
        assert_eq!(
            total,
            field.cell_count(),
            "no cell is lost or counted twice"
        );
        assert!(
            pieces[0].cell_count() > pieces[1].cell_count(),
            "the pieces come back bottom row first, and the 5 mm square is lower"
        );
    }

    #[test]
    fn a_cell_knows_which_piece_it_is_in() {
        let field = square(10.0, 10.0, 5.0).with(&square(25.0, 25.0, 3.0));
        let parts = field.parts();
        assert_eq!(parts.count(), 2);

        let grid = grid();
        let low = grid.cell_of(Vec2::splat(12.0));
        let high = grid.cell_of(Vec2::splat(26.0));
        assert_eq!(
            parts.at(low[0], low[1]),
            Some(0),
            "the lower piece is first"
        );
        assert_eq!(parts.at(high[0], high[1]), Some(1));

        let empty = grid.cell_of(Vec2::splat(20.0));
        assert_eq!(parts.at(empty[0], empty[1]), None, "nothing covers the gap");
    }

    #[test]
    fn a_piece_knows_the_pieces_it_grew_out_of() {
        // Two squares below, one slab over both of them: the slab grew out of the pair.
        let below = square(10.0, 10.0, 5.0).with(&square(20.0, 10.0, 5.0));
        let above = rectangle(10.0, 10.0, 15.0, 5.0);

        let parents = above.parts().parents(&below.parts());
        assert_eq!(parents.len(), 1, "the slab is one piece");
        assert_eq!(parents[0].len(), 2, "and it stands on both squares");
    }

    #[test]
    fn a_piece_standing_on_nothing_has_no_parent() {
        let below = square(10.0, 10.0, 5.0);
        let above = square(10.0, 10.0, 5.0).with(&square(30.0, 30.0, 5.0));

        let parents = above.parts().parents(&below.parts());
        assert_eq!(parents.len(), 2);
        assert_eq!(
            parents[0].len(),
            1,
            "the piece over the square stands on it"
        );
        assert!(parents[1].is_empty(), "the other one is an island");
    }

    #[test]
    fn a_diagonal_chain_of_cells_is_one_piece() {
        // Two squares meeting at a corner only.
        let field = square(10.0, 10.0, 2.0).with(&square(12.0, 12.0, 2.0));
        assert_eq!(field.pieces().len(), 1);
    }
}
